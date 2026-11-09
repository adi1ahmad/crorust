use egui::{
    Key, KeyboardShortcut, Modifiers, ScrollArea, TextBuffer, TextEdit, Ui, text::CCursorRange,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum MobileEditorTab {
    #[default]
    Source,
    Preview,
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Editor {
    code: String,
    highlight_editor: bool,
    show_rendered: bool,
    mobile_tab: MobileEditorTab,

    #[cfg_attr(feature = "serde", serde(skip))]
    highlighter: super::highlighter::MemoizedEasymarkHighlighter,
}

pub type EasyMarkEditor = Editor;

impl PartialEq for Editor {
    fn eq(&self, other: &Self) -> bool {
        (
            &self.code,
            self.highlight_editor,
            self.show_rendered,
            self.mobile_tab,
        ) == (
            &other.code,
            other.highlight_editor,
            other.show_rendered,
            other.mobile_tab,
        )
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            code: DEFAULT_CODE.trim().to_owned(),
            highlight_editor: true,
            show_rendered: true,
            mobile_tab: MobileEditorTab::default(),
            highlighter: Default::default(),
        }
    }
}

impl crate::Demo for Editor {
    fn name(&self) -> &'static str {
        "📝 Markdown Editor"
    }

    fn show(&mut self, ui: &mut egui::Ui, open: &mut bool) {
        let is_compact = crate::ui::ScreenTier::from_ui(ui).is_compact();
        let default_size = if is_compact {
            let screen = ui.ctx().content_rect();
            [screen.width() - 24.0, (screen.height() - 60.0).max(300.0)]
        } else {
            [720.0, 520.0]
        };

        egui::Window::new(self.name())
            .open(open)
            .default_size(default_size)
            .vscroll(false)
            .resizable(true)
            .constrain_to(ui.available_rect_before_wrap())
            .show(ui, |ui| {
                self.ui(ui);
            });
    }
}

impl Editor {
    pub fn panels(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("markdown_editor_bottom").show(ui, |ui| {
            let layout = egui::Layout::top_down(egui::Align::Center).with_main_justify(true);
            ui.allocate_ui_with_layout(ui.available_size(), layout, |ui| {
                ui.label("ruxt Markdown Document Editor");
            })
        });

        egui::CentralPanel::default().show(ui, |ui| {
            self.ui(ui);
        });
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("controls").show(ui, |ui| {
            let _response = ui.button("Hotkeys").on_hover_ui(nested_hotkeys_ui);
            ui.checkbox(&mut self.show_rendered, "Show rendered");
            ui.checkbox(&mut self.highlight_editor, "Highlight editor");
            ui.menu_button("📄 Load Template", |ui| {
                if ui.button("📋 Architecture Decision Record").clicked() {
                    self.code = "# ADR 001: Pure-Rust Cross-Target Packaging\n\n\
## Status\nAccepted\n\n\
## Context\nTraditional toolchains require 30GB+ of platform IDEs.\n\n\
## Decision\nWe compile and sign native artifacts using in-tree member crates.\n\n\
## Consequences\nZero external toolchain dependency; identical commands on all hosts."
                        .to_owned();
                    ui.close();
                }
                if ui.button("📦 Project Release Notes").clicked() {
                    self.code = "# Release v0.1.0\n\n\
## Highlights\n\
- Initial release of *ruxt* cross-platform template\n\
- Full support for Android, iOS, macOS, Windows, Linux, and Web\n\
- Built-in living manual and widget catalog"
                        .to_owned();
                    ui.close();
                }
                if ui.button("📖 Quickstart Guide").clicked() {
                    self.code = "# Getting Started with ruxt\n\n\
1. Run `crorust build -p android` for Android APK\n\
2. Run `crorust build -p windows` for Windows MSIX\n\
3. Run `crorust build -p linux` for Linux AppImage\n\
4. Run `crorust run --device <id>` to launch directly on hardware."
                        .to_owned();
                    ui.close();
                }
            });
            if ui.button("Reset").clicked() {
                *self = Self::default();
            }
            ui.end_row();
        });
        ui.separator();

        let is_compact = crate::ui::ScreenTier::from_ui(ui).is_compact();

        if is_compact && self.show_rendered {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mobile_tab, MobileEditorTab::Source, "✏ Source");
                ui.selectable_value(&mut self.mobile_tab, MobileEditorTab::Preview, "👁 Preview");
            });
            ui.separator();
            match self.mobile_tab {
                MobileEditorTab::Source => {
                    ScrollArea::vertical()
                        .id_salt("source")
                        .show(ui, |ui| self.editor_ui(ui));
                }
                MobileEditorTab::Preview => {
                    ScrollArea::vertical().id_salt("rendered").show(ui, |ui| {
                        super::viewer::easy_mark(ui, &self.code);
                    });
                }
            }
        } else if self.show_rendered {
            ui.columns(2, |columns| {
                ScrollArea::vertical()
                    .id_salt("source")
                    .show(&mut columns[0], |ui| self.editor_ui(ui));
                ScrollArea::vertical()
                    .id_salt("rendered")
                    .show(&mut columns[1], |ui| {
                        super::viewer::easy_mark(ui, &self.code);
                    });
            });
        } else {
            ScrollArea::vertical()
                .id_salt("source")
                .show(ui, |ui| self.editor_ui(ui));
        }
    }

    fn editor_ui(&mut self, ui: &mut egui::Ui) {
        let Self {
            code, highlighter, ..
        } = self;

        let response = if self.highlight_editor {
            let mut layouter = |ui: &egui::Ui, easymark: &dyn TextBuffer, wrap_width: f32| {
                let mut layout_job = highlighter.highlight(ui.style(), easymark.as_str());
                layout_job.wrap.max_width = wrap_width;
                ui.fonts_mut(|f| f.layout_job(layout_job))
            };

            ui.add(
                egui::TextEdit::multiline(code)
                    .hint_text("EasyMark source")
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace)
                    .layouter(&mut layouter),
            )
        } else {
            ui.add(
                egui::TextEdit::multiline(code)
                    .hint_text("EasyMark source")
                    .desired_width(f32::INFINITY),
            )
        };

        if let Some(mut state) = TextEdit::load_state(ui.ctx(), response.id)
            && let Some(mut ccursor_range) = state.cursor.char_range()
        {
            let any_change = shortcuts(ui, code, &mut ccursor_range);
            if any_change {
                state.cursor.set_char_range(Some(ccursor_range));
                state.store(ui.ctx(), response.id);
            }
        }
    }
}

pub const SHORTCUT_BOLD: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::B);
pub const SHORTCUT_CODE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
pub const SHORTCUT_ITALICS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::I);
pub const SHORTCUT_SUBSCRIPT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::L);
pub const SHORTCUT_SUPERSCRIPT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
pub const SHORTCUT_STRIKETHROUGH: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::SHIFT), Key::Q);
pub const SHORTCUT_UNDERLINE: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::SHIFT), Key::W);
pub const SHORTCUT_INDENT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::SHIFT), Key::E);

fn nested_hotkeys_ui(ui: &mut egui::Ui) {
    egui::Grid::new("shortcuts").striped(true).show(ui, |ui| {
        let mut label = |shortcut, what| {
            ui.label(what);
            ui.weak(ui.ctx().format_shortcut(&shortcut));
            ui.end_row();
        };

        label(SHORTCUT_BOLD, "*bold*");
        label(SHORTCUT_CODE, "`code`");
        label(SHORTCUT_ITALICS, "/italics/");
        label(SHORTCUT_SUBSCRIPT, "$subscript$");
        label(SHORTCUT_SUPERSCRIPT, "^superscript^");
        label(SHORTCUT_STRIKETHROUGH, "~strikethrough~");
        label(SHORTCUT_UNDERLINE, "_underline_");
        label(SHORTCUT_INDENT, "two spaces");
    });
}

fn shortcuts(ui: &Ui, code: &mut dyn TextBuffer, ccursor_range: &mut CCursorRange) -> bool {
    let mut any_change = false;

    if ui.input_mut(|i| i.consume_shortcut(&SHORTCUT_INDENT)) {
        any_change = true;
        let [primary, _secondary] = ccursor_range.sorted_cursors();

        let advance = code.insert_text("  ", primary.index);
        ccursor_range.primary.index += advance;
        ccursor_range.secondary.index += advance;
    }

    for (shortcut, surrounding) in [
        (SHORTCUT_BOLD, "*"),
        (SHORTCUT_CODE, "`"),
        (SHORTCUT_ITALICS, "/"),
        (SHORTCUT_SUBSCRIPT, "$"),
        (SHORTCUT_SUPERSCRIPT, "^"),
        (SHORTCUT_STRIKETHROUGH, "~"),
        (SHORTCUT_UNDERLINE, "_"),
    ] {
        if ui.input_mut(|i| i.consume_shortcut(&shortcut)) {
            any_change = true;
            toggle_surrounding(code, ccursor_range, surrounding);
        }
    }

    any_change
}

fn toggle_surrounding(
    code: &mut dyn TextBuffer,
    ccursor_range: &mut CCursorRange,
    surrounding: &str,
) {
    let [primary, secondary] = ccursor_range.sorted_cursors();

    let surrounding_ccount = surrounding.chars().count();

    let prefix_crange = primary.index.saturating_sub(surrounding_ccount)..primary.index;
    let suffix_crange = secondary.index..secondary.index.saturating_add(surrounding_ccount);
    let already_surrounded = code.char_range(prefix_crange.clone()) == surrounding
        && code.char_range(suffix_crange.clone()) == surrounding;

    if already_surrounded {
        code.delete_char_range(suffix_crange);
        code.delete_char_range(prefix_crange);
        ccursor_range.primary.index -= surrounding_ccount;
        ccursor_range.secondary.index -= surrounding_ccount;
    } else {
        code.insert_text(surrounding, secondary.index);
        let advance = code.insert_text(surrounding, primary.index);

        ccursor_range.primary.index += advance;
        ccursor_range.secondary.index += advance;
    }
}

const DEFAULT_CODE: &str = r#"
# EasyMark
EasyMark is a markup language, designed for extreme simplicity.

```
WARNING: EasyMark is still an evolving specification,
and is also missing some features.
```

----------------

# At a glance
- inline text:
  - normal, `code`, *strong*, ~strikethrough~, _underline_, /italics/, ^raised^, $small$
  - `\` escapes the next character
  - [hyperlink](https://github.com/adi1ahmad/crorust)
  - Embedded URL: <https://github.com/adi1ahmad/crorust>
- `# ` header
- `---` separator (horizontal line)
- `> ` quote
- `- ` bullet list
- `1. ` numbered list
- \`\`\` code fence
- a^2^ + b^2^ = c^2^
- $Remember to read the small print$

# Design
> /"Why do what everyone else is doing, when everyone else is already doing it?"
>   \- Emil

Goals:
1. easy to parse
2. easy to learn
3. similar to markdown

[The reference parser](https://github.com/adi1ahmad/crorust/blob/main/templates/crorust.ruxt/src/markdown/parser.rs) is \~250 lines of code, using only the Rust standard library. The parser uses no look-ahead or recursion.

There is never more than one way to accomplish the same thing, and each special character is only used for one thing. For instance `*` is used for *strong* and `-` is used for bullet lists. There is no alternative way to specify the *strong* style or getting a bullet list.

Similarity to markdown is kept when possible, but with much less ambiguity and some improvements (like _underlining_).

# Details
All style changes are single characters, so it is `*strong*`, NOT `**strong**`. Style is reset by a matching character, or at the end of the line.

Style change characters and escapes (`\`) work everywhere except for in inline code, code blocks and in URLs.

You can mix styles. For instance: /italics _underline_/ and *strong `code`*.

You can use styles on URLs: ~my webpage is at <http://www.example.com>~.

Newlines are preserved. If you want to continue text on the same line, just do so. Alternatively, escape the newline by ending the line with a backslash (`\`). \
Escaping the newline effectively ignores it.

The style characters are chosen to be similar to what they are representing:
  `_` = _underline_
  `~` = ~strikethrough~ (`-` is used for bullet points)
  `/` = /italics/
  `*` = *strong*
  `$` = $small$
  `^` = ^raised^

# To do
- Sub-headers (`## h2`, `### h3` etc)
- Hotkey Editor
- International keyboard algorithm for non-letter keys
- ALT+SHIFT+Num1 is not a functioning hotkey
- Tab Indent Increment/Decrement CTRL+], CTRL+[

- Images
  - we want to be able to optionally specify size (width and\/or height)
  - centering of images is very desirable
  - captioning (image with a text underneath it)
  - `![caption=My image][width=200][center](url)` ?
- Nicer URL:s
  - `<url>` and `[url](url)` do the same thing yet look completely different.
  - let's keep similarity with images
- Tables
- Inspiration: <https://mycorrhiza.wiki/help/en/mycomarkup>
"#;
