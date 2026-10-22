use crate::model::dependency::DependencyRequirement;
use crate::model::pom::ProjectObjectModel;
use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::BufRead;

pub struct StreamingPomReader;

impl StreamingPomReader {
    pub fn parse_pom_stream<R: BufRead>(reader: R) -> Result<ProjectObjectModel> {
        let mut xml = Reader::from_reader(reader);
        xml.config_mut().trim_text(true);

        let mut buf = Vec::with_capacity(1024);
        let mut current_tag = String::new();
        let mut in_parent = false;
        let mut in_dependency = false;

        let mut version = None;
        let mut packaging = "jar".to_string();
        let mut dependencies = Vec::new();

        let mut dep_group = String::new();
        let mut dep_artifact = String::new();
        let mut dep_version = None;
        let mut dep_scope = None;
        let mut dep_optional = None;

        loop {
            match xml.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    current_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if current_tag == "parent" {
                        in_parent = true;
                    }
                    if current_tag == "dependency" {
                        in_dependency = true;
                    }
                }
                Ok(Event::Text(e)) => {
                    let text = e.unescape()?.to_string();
                    if in_parent {
                        if current_tag == "version" && version.is_none() {
                            version = Some(text);
                        }
                    } else if in_dependency {
                        match current_tag.as_str() {
                            "groupId" => dep_group = text,
                            "artifactId" => dep_artifact = text,
                            "version" => dep_version = Some(text),
                            "scope" => dep_scope = Some(text),
                            "optional" => dep_optional = text.parse::<bool>().ok(),
                            _ => {}
                        }
                    } else {
                        match current_tag.as_str() {
                            "version" => version = Some(text),
                            "packaging" => packaging = text,
                            _ => {}
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    let name = e.name();
                    let tag = String::from_utf8_lossy(name.as_ref());
                    if tag == "dependency" {
                        dependencies.push(DependencyRequirement {
                            group: std::mem::take(&mut dep_group).into(),
                            name: std::mem::take(&mut dep_artifact).into(),
                            version: dep_version.take(),
                            scope: dep_scope.take(),
                            optional: dep_optional.take(),
                        });
                        in_dependency = false;
                    }
                    if tag == "parent" {
                        in_parent = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(err) => anyhow::bail!("XML parse error: {err:?}"),
                _ => {}
            }
            buf.clear();
        }

        Ok(ProjectObjectModel {
            version,
            packaging: Some(packaging),
            dependencies: crate::model::pom::DependencyContainer { dependencies },
            dependency_management: Default::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_pom_reader_dependencies() {
        let xml = r#"<project>
            <groupId>com.example</groupId>
            <artifactId>test-project</artifactId>
            <version>1.2.3</version>
            <packaging>aar</packaging>
            <dependencies>
                <dependency>
                    <groupId>androidx.core</groupId>
                    <artifactId>core-ktx</artifactId>
                    <version>1.12.0</version>
                    <scope>compile</scope>
                </dependency>
            </dependencies>
        </project>"#;

        let pom = StreamingPomReader::parse_pom_stream(xml.as_bytes()).unwrap();
        assert_eq!(pom.version.as_deref(), Some("1.2.3"));
        assert_eq!(pom.packaging(), "aar");
        let deps = pom.dependencies();
        assert_eq!(deps.len(), 1);
        assert_eq!(&*deps[0].group, "androidx.core");
        assert_eq!(&*deps[0].name, "core-ktx");
        assert_eq!(deps[0].version.as_deref(), Some("1.12.0"));
    }
}
