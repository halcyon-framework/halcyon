//! HAL390: normal public native reader. No parser, shaper or painter in the Screen.
use halcyon::{
    core::{
        app::{App, AppConfig, Screen, ScreenFrameContext},
        frame_realization::AppFrameObserver,
        layout::{EdgeInsets, FlexParams},
        text_selection::TextSelectionPolicy,
        widget::Widget,
    },
    platform::font_manager::{FontAsset, FontManager, FontWeight},
    prelude::Label,
    widgets::{
        flex::Flex,
        markdown::{MarkdownDocument, MarkdownLimits, MarkdownStyle, MarkdownView},
        scroll_area::ScrollArea,
    },
};
use std::sync::Arc;

const DOCUMENT: &str = r#"# Native Markdown reader

An immutable **source-bearing** document with *real italic*, ***bold italic***,
~~strikethrough~~ and `inline code`. Select text; copy reading text with Cmd/Ctrl+C
or exact Markdown with Cmd/Ctrl+Shift+C. Both scroll axes use one ScrollArea.

## Lists and quotations

3. Preserve the exact source.
4. Keep native text and input owners.
   - [x] Read-only task status
   - [ ] No content execution

> A quiet quotation with **native typography**.
>
> Latin, Привет, 看龍, אבג and é remain original Unicode.

## Wide code — scroll horizontally

```rust
fn source_is_immutable() {
	let columns = ["source", "reading", "literal", "styles", "bindings", "selection", "layout", "compile", "native output"];
	assert!(columns.len() > 0);
}
```

## Table with genuine natural extent

| Meaning | Exact value | Native owner |
| :--- | :---: | ---: |
| Source | **byte-identical** | MarkdownDocument |
| Reading | decoded &amp; selectable | MarkdownView |
| Code | literal tabs retained | Registered Courier New |
| Wide content | reachable on either window size | One ScrollArea |

### Passive content

[An inert link](https://example.invalid/never-opened)
and ![a non-loaded image](file:never-loaded).

<script>this is literal text, never executed</script>

---

The foreground remains ordinary native text. No model, WebView or image service.
"#;

#[cfg(target_os = "macos")]
fn assets() -> Result<Vec<FontAsset>, Box<dyn std::error::Error>> {
    let mut manager = FontManager::new();
    let mut result = Vec::new();
    for (file, family, weight, italic) in [
        ("Arial.ttf", "Arial", FontWeight::Regular, false),
        ("Arial Bold.ttf", "Arial", FontWeight::Bold, false),
        ("Arial Italic.ttf", "Arial", FontWeight::Regular, true),
        ("Arial Bold Italic.ttf", "Arial", FontWeight::Bold, true),
        ("Courier New.ttf", "Courier New", FontWeight::Regular, false),
        (
            "Courier New Bold.ttf",
            "Courier New",
            FontWeight::Bold,
            false,
        ),
        (
            "Courier New Italic.ttf",
            "Courier New",
            FontWeight::Regular,
            true,
        ),
        (
            "Courier New Bold Italic.ttf",
            "Courier New",
            FontWeight::Bold,
            true,
        ),
        (
            "Arial Unicode.ttf",
            "Arial Unicode MS",
            FontWeight::Regular,
            false,
        ),
    ] {
        let id = manager.load_font_file(&format!("/System/Library/Fonts/Supplemental/{file}"))?;
        let face = manager.get_face(id).ok_or("loaded font missing")?;
        let parsed = ttf_parser::Face::parse(&face.data, 0)?;
        let actual_family = parsed
            .names()
            .into_iter()
            .filter(|n| n.name_id == ttf_parser::name_id::FAMILY)
            .find_map(|n| n.to_string())
            .ok_or("font family metadata unavailable")?;
        if actual_family != family
            || FontWeight::from_value(parsed.weight().to_number()) != weight
            || parsed.is_italic() != italic
        {
            return Err("font metadata does not match selected role".into());
        }
        result.push(FontAsset {
            family_name: actual_family,
            weight: face.weight,
            italic: face.italic,
            data: face.data.clone(),
        });
    }
    Ok(result)
}
struct Reader {
    document: Arc<MarkdownDocument>,
    placeholder: Label,
    observer: AppFrameObserver,
    reported: bool,
}
impl Screen for Reader {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column().with_padding(EdgeInsets::all(20.0))));
        cx.tree.set_flex(root, FlexParams::flexible(1.0));
        let scroll = cx.tree.add_child(
            root,
            Box::new(ScrollArea::new_unwired().enable_horizontal_scrollbar(true)),
            None,
            0,
        );
        cx.tree.set_flex(scroll, FlexParams::flexible(1.0));
        let reader = cx.tree.add_child(
            scroll,
            Box::new(
                MarkdownView::new(
                    self.document.clone(),
                    MarkdownStyle::new("Arial", "Courier New").unwrap(),
                )
                .with_selection_policy(TextSelectionPolicy::Enabled),
            ),
            None,
            0,
        );
        cx.tree
            .node_mut(scroll)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(reader);
        &mut self.placeholder
    }
    fn take_rebuild_flag(&mut self) -> bool {
        false
    }
    fn update(&mut self, _tree: &mut halcyon::core::widget_tree::WidgetTree) {
        // Observe only an already completed ordinary attempt during an existing
        // update. This requests no tick/redraw, retains no history and emits once.
        if !self.reported {
            if let Some(receipt) = self.observer.latest() {
                if receipt.surfaces().iter().any(|surface| {
                    surface.output_produced() && surface.submitted() && surface.presented_called()
                }) {
                    eprintln!("HAL390_NATIVE_FRAME {receipt:?}");
                    self.reported = true;
                }
            }
        }
    }
}
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fonts = assets()?;
    let document = MarkdownDocument::parse(DOCUMENT.into(), MarkdownLimits::default())?;
    let (config, observer) = AppConfig::new("Native Markdown - HAL390")
        .with_size(980.0, 800.0)
        .with_font_assets(fonts)
        .with_frame_realization_observer();
    App::new(config).run(Reader {
        document,
        placeholder: Label::new(""),
        observer,
        reported: false,
    });
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This example requires the explicitly selected installed macOS font assets.");
}
