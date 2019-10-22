//! The core layouting engine.

use std::borrow::Cow;
use std::io::{self, Write};

use toddle::query::{FontClass, SharedFontLoader};
use toddle::Error as FontError;

use crate::func::Command;
use crate::size::{Size, Size2D, SizeBox};
use crate::style::TextStyle;
use crate::syntax::{FuncCall, Node, SyntaxTree};

mod actions;
mod tree;
mod flex;
mod stacked;
mod text;

/// Different kinds of layouters (fully re-exported).
pub mod layouters {
    pub use super::tree::layout_tree;
    pub use super::flex::{FlexLayouter, FlexContext};
    pub use super::stacked::{StackLayouter, StackContext};
    pub use super::text::{layout_text, TextContext};
}

pub use actions::{LayoutAction, LayoutActionList};
pub use layouters::*;

/// A sequence of layouting actions inside a box.
#[derive(Debug, Clone)]
pub struct Layout {
    /// The size of the box.
    pub dimensions: Size2D,
    /// The actions composing this layout.
    pub actions: Vec<LayoutAction>,
    /// Whether to debug-render this box.
    pub debug_render: bool,
}

impl Layout {
    /// Create an empty layout with the specified dimensions.
    pub fn empty(width: Size, height: Size) -> Layout {
        Layout {
            dimensions: Size2D::new(width, height),
            actions: vec![],
            debug_render: true,
        }
    }

    /// Serialize this layout into an output buffer.
    pub fn serialize<W: Write>(&self, f: &mut W) -> io::Result<()> {
        writeln!(
            f,
            "{:.4} {:.4}",
            self.dimensions.x.to_pt(),
            self.dimensions.y.to_pt()
        )?;
        writeln!(f, "{}", self.actions.len())?;
        for action in &self.actions {
            action.serialize(f)?;
            writeln!(f)?;
        }
        Ok(())
    }
}

/// A collection of layouts.
#[derive(Debug, Clone)]
pub struct MultiLayout {
    pub layouts: Vec<Layout>,
}

impl MultiLayout {
    /// Create an empty multi-layout.
    pub fn new() -> MultiLayout {
        MultiLayout { layouts: vec![] }
    }

    /// Extract the single sublayout. This panics if the layout does not have
    /// exactly one child.
    pub fn into_single(mut self) -> Layout {
        if self.layouts.len() != 1 {
            panic!("into_single: contains not exactly one layout");
        }
        self.layouts.pop().unwrap()
    }

    /// Add a sublayout.
    pub fn add(&mut self, layout: Layout) {
        self.layouts.push(layout);
    }

    /// The count of sublayouts.
    pub fn count(&self) -> usize {
        self.layouts.len()
    }

    /// Whether this layout contains any sublayouts.
    pub fn is_empty(&self) -> bool {
        self.layouts.is_empty()
    }
}

impl MultiLayout {
    /// Serialize this collection of layouts into an output buffer.
    pub fn serialize<W: Write>(&self, f: &mut W) -> io::Result<()> {
        writeln!(f, "{}", self.count())?;
        for layout in self {
            layout.serialize(f)?;
        }
        Ok(())
    }
}

impl IntoIterator for MultiLayout {
    type Item = Layout;
    type IntoIter = std::vec::IntoIter<Layout>;

    fn into_iter(self) -> Self::IntoIter {
        self.layouts.into_iter()
    }
}

impl<'a> IntoIterator for &'a MultiLayout {
    type Item = &'a Layout;
    type IntoIter = std::slice::Iter<'a, Layout>;

    fn into_iter(self) -> Self::IntoIter {
        self.layouts.iter()
    }
}

/// The general context for layouting.
#[derive(Debug, Copy, Clone)]
pub struct LayoutContext<'a, 'p> {
    /// The font loader to retrieve fonts from when typesetting text
    /// using [`layout_text`].
    pub loader: &'a SharedFontLoader<'p>,
    /// The style to set text with. This includes sizes and font classes
    /// which determine which font from the loaders selection is used.
    pub style: &'a TextStyle,
    /// The alignment to use for the content.
    pub alignment: Alignment,
    /// How to stack the context.
    pub flow: Flow,
    /// The primary space to layout in.
    pub space: LayoutSpace,
    /// The additional spaces which are used when the primary space
    /// cannot fit the whole content.
    pub followup_spaces: Option<LayoutSpace>,
    /// Whether to shrink the dimensions to fit the content or the keep the
    /// dimensions from the layout spaces.
    pub shrink_to_fit: bool,
}

/// Spacial layouting constraints.
#[derive(Debug, Copy, Clone)]
pub struct LayoutSpace {
    /// The maximum size of the box to layout in.
    pub dimensions: Size2D,
    /// Padding that should be respected on each side.
    pub padding: SizeBox,
}

impl LayoutSpace {
    /// The actually usable area (dimensions minus padding).
    pub fn usable(&self) -> Size2D {
        self.dimensions.unpadded(self.padding)
    }

    /// A layout without padding and dimensions reduced by the padding.
    pub fn usable_space(&self) -> LayoutSpace {
        LayoutSpace {
            dimensions: self.usable(),
            padding: SizeBox::zero(),
        }
    }
}

/// Where to align content.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Alignment {
    Left,
    Right,
    Center,
}

/// The flow of content.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Flow {
    Vertical,
    Horizontal,
}

/// The error type for layouting.
pub enum LayoutError {
    /// There is not enough space to add an item.
    NotEnoughSpace(&'static str),
    /// There was no suitable font for the given character.
    NoSuitableFont(char),
    /// An error occured while gathering font data.
    Font(FontError),
}

/// The result type for layouting.
pub type LayoutResult<T> = Result<T, LayoutError>;

error_type! {
    err: LayoutError,
    show: f => match err {
        LayoutError::NotEnoughSpace(desc) => write!(f, "not enough space: {}", desc),
        LayoutError::NoSuitableFont(c) => write!(f, "no suitable font for '{}'", c),
        LayoutError::Font(err) => write!(f, "font error: {}", err),
    },
    source: match err {
        LayoutError::Font(err) => Some(err),
        _ => None,
    },
    from: (std::io::Error, LayoutError::Font(FontError::Io(err))),
    from: (FontError, LayoutError::Font(err)),
}
