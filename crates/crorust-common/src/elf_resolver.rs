use anyhow::{bail, Context, Result};
use std::collections::{HashSet, VecDeque};
use std::ffi::{OsStr, OsString};
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DynamicLibraryResolution {
    pub resolved_paths: HashSet<PathBuf>,
    pub requires_ndk_cxx_runtime: bool,
}

pub fn trace_dynamic_dependencies(
    target_binary: &Path,
    search_directories: &[&Path],
    system_provided: &HashSet<OsString>,
) -> Result<DynamicLibraryResolution> {
    let mut resolved_paths = HashSet::new();
    let mut traversed_paths = HashSet::new();
    let mut requires_ndk_cxx_runtime = false;

    if let Ok(canonical_target) = dunce::canonicalize(target_binary) {
        traversed_paths.insert(canonical_target);
    }

    let mut pending_queue = VecDeque::new();
    pending_queue.push_back(target_binary.to_path_buf());

    while let Some(current_artifact) = pending_queue.pop_front() {
        let needed_entries = query_dt_needed_entries(&current_artifact).with_context(|| {
            format!(
                "Failed inspecting dynamic dependencies for ELF binary: {}",
                current_artifact.display()
            )
        })?;

        for soname in needed_entries {
            if soname == "libc++_shared.so" {
                requires_ndk_cxx_runtime = true;
            } else if !system_provided.contains(OsStr::new(&soname)) {
                let resolved_path = locate_shared_object(search_directories, &soname)
                    .with_context(|| {
                        format!("Failed scanning search directories for shared library: {soname}")
                    })?;

                match resolved_path {
                    Some(full_path) => {
                        resolved_paths.insert(full_path.clone());
                        if traversed_paths.insert(full_path.clone()) {
                            pending_queue.push_back(full_path);
                        }
                    }
                    None => {
                        bail!("Required shared library `{soname}` was not found in sysroot search paths");
                    }
                }
            }
        }
    }

    Ok(DynamicLibraryResolution {
        resolved_paths,
        requires_ndk_cxx_runtime,
    })
}

pub fn query_dt_needed_entries(library_path: &Path) -> Result<HashSet<String>> {
    let file = std::fs::File::open(library_path)
        .with_context(|| format!("Failed opening ELF library at {}", library_path.display()))?;
    let mut reader = BufReader::new(file);
    read_elf_dt_needed_stream(&mut reader)
}

pub fn read_elf_dt_needed_direct(elf_bytes: &[u8]) -> Option<HashSet<String>> {
    read_elf_dt_needed_stream(Cursor::new(elf_bytes)).ok()
}

pub fn read_elf_dt_needed_stream<R: Read + Seek>(mut reader: R) -> Result<HashSet<String>> {
    let mut e_ident = [0u8; 16];
    reader
        .read_exact(&mut e_ident)
        .context("Failed reading ELF identification header")?;

    if &e_ident[0..4] != b"\x7fELF" {
        bail!("Not an ELF binary: invalid magic signature");
    }

    let is_64_bit = match e_ident[4] {
        1 => false,
        2 => true,
        other => bail!("Unsupported ELF class: {}", other),
    };

    let is_little_endian = match e_ident[5] {
        1 => true,
        2 => false,
        other => bail!("Unsupported ELF data encoding: {}", other),
    };

    if is_64_bit {
        parse_elf64_stream(&mut reader, is_little_endian)
    } else {
        parse_elf32_stream(&mut reader, is_little_endian)
    }
}

fn parse_elf64_stream<R: Read + Seek>(reader: &mut R, le: bool) -> Result<HashSet<String>> {
    reader.seek(SeekFrom::Start(32))?;
    let mut phoff_bytes = [0u8; 8];
    reader.read_exact(&mut phoff_bytes)?;
    let phoff = if le {
        u64::from_le_bytes(phoff_bytes)
    } else {
        u64::from_be_bytes(phoff_bytes)
    };

    reader.seek(SeekFrom::Start(54))?;
    let mut ph_counts = [0u8; 4];
    reader.read_exact(&mut ph_counts)?;
    let phentsize = if le {
        u16::from_le_bytes(ph_counts[0..2].try_into().unwrap()) as usize
    } else {
        u16::from_be_bytes(ph_counts[0..2].try_into().unwrap()) as usize
    };
    let phnum = if le {
        u16::from_le_bytes(ph_counts[2..4].try_into().unwrap()) as usize
    } else {
        u16::from_be_bytes(ph_counts[2..4].try_into().unwrap()) as usize
    };

    let mut dynamic_segment: Option<(u64, u64)> = None;
    let mut load_segments: Vec<(u64, u64, u64)> = Vec::new();

    for idx in 0..phnum {
        let seg_start = phoff
            .checked_add((idx * phentsize) as u64)
            .context("ELF program header offset overflow")?;
        reader.seek(SeekFrom::Start(seg_start))?;

        let mut ph = [0u8; 56];
        if reader.read_exact(&mut ph).is_err() {
            break;
        }

        let p_type = if le {
            u32::from_le_bytes(ph[0..4].try_into().unwrap())
        } else {
            u32::from_be_bytes(ph[0..4].try_into().unwrap())
        };
        let p_offset = if le {
            u64::from_le_bytes(ph[8..16].try_into().unwrap())
        } else {
            u64::from_be_bytes(ph[8..16].try_into().unwrap())
        };
        let p_vaddr = if le {
            u64::from_le_bytes(ph[16..24].try_into().unwrap())
        } else {
            u64::from_be_bytes(ph[16..24].try_into().unwrap())
        };
        let p_filesz = if le {
            u64::from_le_bytes(ph[32..40].try_into().unwrap())
        } else {
            u64::from_be_bytes(ph[32..40].try_into().unwrap())
        };

        if p_type == 1 {
            load_segments.push((p_vaddr, p_offset, p_filesz));
        } else if p_type == 2 {
            dynamic_segment = Some((p_offset, p_filesz));
        }
    }

    let (dyn_offset, dyn_size) = dynamic_segment.context("No PT_DYNAMIC segment found in ELF")?;
    reader.seek(SeekFrom::Start(dyn_offset))?;

    let mut strtab_vaddr = None;
    let mut needed_offsets = Vec::new();
    let mut bytes_read = 0u64;

    while bytes_read + 16 <= dyn_size {
        let mut entry = [0u8; 16];
        if reader.read_exact(&mut entry).is_err() {
            break;
        }
        bytes_read += 16;

        let d_tag = if le {
            i64::from_le_bytes(entry[0..8].try_into().unwrap())
        } else {
            i64::from_be_bytes(entry[0..8].try_into().unwrap())
        };
        let d_val = if le {
            u64::from_le_bytes(entry[8..16].try_into().unwrap())
        } else {
            u64::from_be_bytes(entry[8..16].try_into().unwrap())
        };

        match d_tag {
            0 => break, // DT_NULL
            1 => needed_offsets.push(d_val),
            5 => strtab_vaddr = Some(d_val),
            _ => {}
        }
    }

    let strtab_address = strtab_vaddr.context("Missing DT_STRTAB in ELF dynamic section")?;
    let mut strtab_file_offset = None;
    for (vaddr, file_off, seg_sz) in &load_segments {
        if strtab_address >= *vaddr && strtab_address < *vaddr + *seg_sz {
            strtab_file_offset = Some(*file_off + (strtab_address - *vaddr));
            break;
        }
    }

    let table_start = strtab_file_offset.unwrap_or(strtab_address);
    let mut sonames = HashSet::new();

    for string_off in needed_offsets {
        reader.seek(SeekFrom::Start(table_start + string_off))?;
        let mut name_buf = Vec::new();
        let mut byte = [0u8; 1];
        while reader.read_exact(&mut byte).is_ok() && byte[0] != 0 {
            name_buf.push(byte[0]);
            if name_buf.len() > 1024 {
                break;
            }
        }
        if let Ok(soname_str) = String::from_utf8(name_buf) {
            sonames.insert(soname_str);
        }
    }

    Ok(sonames)
}

fn parse_elf32_stream<R: Read + Seek>(reader: &mut R, le: bool) -> Result<HashSet<String>> {
    reader.seek(SeekFrom::Start(28))?;
    let mut phoff_bytes = [0u8; 4];
    reader.read_exact(&mut phoff_bytes)?;
    let phoff = if le {
        u32::from_le_bytes(phoff_bytes) as u64
    } else {
        u32::from_be_bytes(phoff_bytes) as u64
    };

    reader.seek(SeekFrom::Start(42))?;
    let mut ph_counts = [0u8; 4];
    reader.read_exact(&mut ph_counts)?;
    let phentsize = if le {
        u16::from_le_bytes(ph_counts[0..2].try_into().unwrap()) as usize
    } else {
        u16::from_be_bytes(ph_counts[0..2].try_into().unwrap()) as usize
    };
    let phnum = if le {
        u16::from_le_bytes(ph_counts[2..4].try_into().unwrap()) as usize
    } else {
        u16::from_be_bytes(ph_counts[2..4].try_into().unwrap()) as usize
    };

    let mut dynamic_segment: Option<(u64, u64)> = None;
    let mut load_segments: Vec<(u64, u64, u64)> = Vec::new();

    for idx in 0..phnum {
        let seg_start = phoff
            .checked_add((idx * phentsize) as u64)
            .context("ELF32 program header offset overflow")?;
        reader.seek(SeekFrom::Start(seg_start))?;

        let mut ph = [0u8; 32];
        if reader.read_exact(&mut ph).is_err() {
            break;
        }

        let p_type = if le {
            u32::from_le_bytes(ph[0..4].try_into().unwrap())
        } else {
            u32::from_be_bytes(ph[0..4].try_into().unwrap())
        };
        let p_offset = if le {
            u32::from_le_bytes(ph[4..8].try_into().unwrap()) as u64
        } else {
            u32::from_be_bytes(ph[4..8].try_into().unwrap()) as u64
        };
        let p_vaddr = if le {
            u32::from_le_bytes(ph[8..12].try_into().unwrap()) as u64
        } else {
            u32::from_be_bytes(ph[8..12].try_into().unwrap()) as u64
        };
        let p_filesz = if le {
            u32::from_le_bytes(ph[16..20].try_into().unwrap()) as u64
        } else {
            u32::from_be_bytes(ph[16..20].try_into().unwrap()) as u64
        };

        if p_type == 1 {
            load_segments.push((p_vaddr, p_offset, p_filesz));
        } else if p_type == 2 {
            dynamic_segment = Some((p_offset, p_filesz));
        }
    }

    let (dyn_offset, dyn_size) = dynamic_segment.context("No PT_DYNAMIC segment found in ELF32")?;
    reader.seek(SeekFrom::Start(dyn_offset))?;

    let mut strtab_vaddr = None;
    let mut needed_offsets = Vec::new();
    let mut bytes_read = 0u64;

    while bytes_read + 8 <= dyn_size {
        let mut entry = [0u8; 8];
        if reader.read_exact(&mut entry).is_err() {
            break;
        }
        bytes_read += 8;

        let d_tag = if le {
            i32::from_le_bytes(entry[0..4].try_into().unwrap())
        } else {
            i32::from_be_bytes(entry[0..4].try_into().unwrap())
        };
        let d_val = if le {
            u32::from_le_bytes(entry[4..8].try_into().unwrap()) as u64
        } else {
            u32::from_be_bytes(entry[4..8].try_into().unwrap()) as u64
        };

        match d_tag {
            0 => break, // DT_NULL
            1 => needed_offsets.push(d_val),
            5 => strtab_vaddr = Some(d_val),
            _ => {}
        }
    }

    let strtab_address = strtab_vaddr.context("Missing DT_STRTAB in ELF32 dynamic section")?;
    let mut strtab_file_offset = None;
    for (vaddr, file_off, seg_sz) in &load_segments {
        if strtab_address >= *vaddr && strtab_address < *vaddr + *seg_sz {
            strtab_file_offset = Some(*file_off + (strtab_address - *vaddr));
            break;
        }
    }

    let table_start = strtab_file_offset.unwrap_or(strtab_address);
    let mut sonames = HashSet::new();

    for string_off in needed_offsets {
        reader.seek(SeekFrom::Start(table_start + string_off))?;
        let mut name_buf = Vec::new();
        let mut byte = [0u8; 1];
        while reader.read_exact(&mut byte).is_ok() && byte[0] != 0 {
            name_buf.push(byte[0]);
            if name_buf.len() > 1024 {
                break;
            }
        }
        if let Ok(soname_str) = String::from_utf8(name_buf) {
            sonames.insert(soname_str);
        }
    }

    Ok(sonames)
}

pub fn scan_directory_for_libraries(directory: &Path) -> Result<HashSet<OsString>> {
    let mut library_names = HashSet::new();
    let dir_reader = std::fs::read_dir(directory)?;

    for entry in dir_reader {
        let item = entry?;
        let full = item.path();
        if !full.is_dir() {
            let filename = item.file_name();
            let lossy = filename.to_string_lossy();
            if full.extension() == Some(OsStr::new("so")) || lossy.contains(".so.") {
                library_names.insert(filename);
            }
        }
    }

    Ok(library_names)
}

pub fn locate_shared_object(directories: &[&Path], soname: &str) -> Result<Option<PathBuf>> {
    for base in directories {
        let candidate = base.join(soname);
        if candidate.exists() {
            return Ok(Some(dunce::canonicalize(candidate)?));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_elf_dt_needed_stream_synthetic_elf64() {
        let mut elf = vec![0u8; 512];
        elf[0..4].copy_from_slice(b"\x7fELF");
        elf[4] = 2; // 64-bit
        elf[5] = 1; // little endian
        elf[6] = 1; // version

        // e_phoff at 32: 64
        elf[32..40].copy_from_slice(&64u64.to_le_bytes());
        // e_phentsize at 54: 56
        elf[54..56].copy_from_slice(&56u16.to_le_bytes());
        // e_phnum at 56: 2
        elf[56..58].copy_from_slice(&2u16.to_le_bytes());

        // Phdr 0: PT_LOAD at 64
        elf[64..68].copy_from_slice(&1u32.to_le_bytes());
        elf[72..80].copy_from_slice(&0u64.to_le_bytes());
        elf[80..88].copy_from_slice(&0x1000u64.to_le_bytes());
        elf[96..104].copy_from_slice(&512u64.to_le_bytes());

        // Phdr 1: PT_DYNAMIC at 120
        elf[120..124].copy_from_slice(&2u32.to_le_bytes());
        elf[128..136].copy_from_slice(&200u64.to_le_bytes());
        elf[136..144].copy_from_slice(&0x10c8u64.to_le_bytes());
        elf[152..160].copy_from_slice(&64u64.to_le_bytes());

        // Dynamic entries at 200:
        // DT_STRTAB (5) -> vaddr 0x1130 (offset 304)
        elf[200..208].copy_from_slice(&5i64.to_le_bytes());
        elf[208..216].copy_from_slice(&0x1130u64.to_le_bytes());

        // DT_NEEDED (1) -> string offset 1 ("libtest.so")
        elf[216..224].copy_from_slice(&1i64.to_le_bytes());
        elf[224..232].copy_from_slice(&1u64.to_le_bytes());

        // DT_NEEDED (1) -> string offset 12 ("libmath.so")
        elf[232..240].copy_from_slice(&1i64.to_le_bytes());
        elf[240..248].copy_from_slice(&12u64.to_le_bytes());

        // DT_NULL (0)
        elf[248..256].copy_from_slice(&0i64.to_le_bytes());
        elf[256..264].copy_from_slice(&0u64.to_le_bytes());

        // String table at 304:
        let strtab = b"\0libtest.so\0libmath.so\0";
        elf[304..304 + strtab.len()].copy_from_slice(strtab);

        let parsed = read_elf_dt_needed_direct(&elf).expect("parsed synthetic ELF64");
        assert_eq!(parsed.len(), 2);
        assert!(parsed.contains("libtest.so"));
        assert!(parsed.contains("libmath.so"));
    }
}
