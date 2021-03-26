//! The genome editor provides the facilities for the player to manipulate their own DNA, provided
//! they have a plasmid that allows this.

use crate::entity::genetics::{Dna, TraitFamily};
use crate::game::{RunState, HUD_CON};
use crate::ui::color::Color;
use crate::ui::color_palette::ColorPalette;
use crate::util::modulus;
use rltk::{to_cp437, ColorPair, DrawBatch, Point, Rect, Rltk, VirtualKeyCode};
use std::fmt;
use std::fmt::{Display, Formatter};

/// Determines which features of the editor are enabled.
#[derive(Debug)]
pub enum GenomeEditorFeatureSet {
    Reduce,  // moving and cutting
    InPlace, // editing without moving or extending, like flipping bits
    Extend,  // copy and adding new
}

#[derive(Debug, Clone, Copy)]
pub enum GenomeEditingState {
    Idle,
    Move,
    Cut,
    FlipBit,
    Duplicate,
    Add,
    Done,
}

impl Display for GenomeEditingState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            GenomeEditingState::Idle => {
                write!(f, "")
            }
            GenomeEditingState::Move => {
                write!(f, "Move")
            }
            GenomeEditingState::Cut => {
                write!(f, "Cut")
            }
            GenomeEditingState::FlipBit => {
                write!(f, "Flip a Bit")
            }
            GenomeEditingState::Duplicate => {
                write!(f, "Duplicate")
            }
            GenomeEditingState::Add => {
                write!(f, "Add new")
            }
            GenomeEditingState::Done => {
                write!(f, "Done")
            }
        }
    }
}

#[derive(Debug)]
struct EditFunction {
    layout: Rect,
    state: GenomeEditingState,
    idx: usize,
    title: String,
}

impl EditFunction {
    fn new(layout: Rect, state: GenomeEditingState, idx: usize, title: String) -> Self {
        EditFunction {
            layout,
            state,
            idx,
            title,
        }
    }
}

#[derive(Debug)]
struct GeneItem {
    layout: Rect,
    /// position of the represented gene within the genome
    gene_idx: usize,
    color: ColorPair,
}

impl GeneItem {
    fn new(layout: Rect, gene_idx: usize, color: ColorPair) -> Self {
        GeneItem {
            layout,
            gene_idx,
            color,
        }
    }
}

/// Layout from top to bottom:
/// - row of buttons for edit functions
/// - row of DNA, rendered like in the sidebar
/// - space for detailed view of the gene
///   - complete binary representation
///   - name and family of the trait
///   - values and additional info about the trait
#[derive(Debug)]
pub struct GenomeEditor {
    layout: Rect,
    features: GenomeEditorFeatureSet,
    active_item: usize,
    pub current_state: GenomeEditingState,
    pub player_dna: Dna,
    clipboard: Option<GeneItem>,
    edit_functions: Vec<EditFunction>,
    gene_items: Vec<GeneItem>,
}

// TODO: Take into account limited 'charges' of plasmids.
impl GenomeEditor {
    // required functions
    // - constructor
    // - render items
    // - methods for each edit function
    //   - move, cut, flip bit, copy, add new

    /// Creates a new instance of the editor. DNA is parsed into items that can be cycled through.
    pub fn new(
        dna: Dna,
        layout: Rect,
        features: GenomeEditorFeatureSet,
        cp: &ColorPalette,
    ) -> Self {
        let mut top_row_x: i32 = 11;
        let top_row_y: i32 = 6;
        let mut mid_row_x: i32 = 11;
        let mid_row_y: i32 = 14;
        let mut gene_items: Vec<GeneItem> = vec![];

        use GenomeEditingState::*;
        let edit_functions: Vec<EditFunction> = [Move, Cut, FlipBit, Duplicate, Add, Done]
            .iter()
            .enumerate()
            .zip(
                [
                    "Move",
                    "Cut",
                    "Mutate",
                    "Duplicate",
                    "Add New",
                    "Confirm",
                    "Cancel",
                ]
                .iter(),
            )
            .map(|((idx, e), s)| {
                let len: i32 = (s.len() + 3) as i32;
                let item = EditFunction::new(
                    Rect::with_size(top_row_x, top_row_y, len, 2),
                    *e,
                    idx,
                    s.to_string(),
                );
                top_row_x += len + 2;
                item
            })
            .collect();

        let gene_items: Vec<GeneItem> = dna
            .simplified
            .iter()
            .enumerate()
            .map(|(idx, i)| {
                let col: (u8, u8, u8) = match i.trait_family {
                    TraitFamily::Sensing => cp.cyan,
                    TraitFamily::Processing => cp.magenta,
                    TraitFamily::Actuating => cp.yellow,
                    TraitFamily::Junk => (100, 100, 100), // TODO
                    TraitFamily::Ltr => (255, 255, 255),  // TODO
                };
                let item = GeneItem::new(
                    Rect::with_size(mid_row_x, mid_row_y, 1, 1),
                    idx,
                    ColorPair::new(col, cp.bg_dna),
                );
                mid_row_x += 1;
                item
            })
            .collect();

        GenomeEditor {
            layout,
            features,
            active_item: 0,
            current_state: GenomeEditingState::Idle,
            player_dna: dna,
            clipboard: None,
            edit_functions, // TODO: Create items and insert here!
            gene_items,     // TODO: Create items and insert here!
        }
    }

    pub fn display(self, ctx: &mut Rltk, palette: &ColorPalette) -> RunState {
        // 1. render everything
        self.render(ctx, palette);

        // 2. read user input
        if let Some(key) = ctx.key {
            return match key {
                VirtualKeyCode::Escape => RunState::CheckInput,
                _ => RunState::GenomeEditing(self),
            };
        }

        // 3. make adjustments to genome

        // 4. package editor back up and return it
        RunState::GenomeEditing(self)
    }

    fn render(&self, ctx: &mut Rltk, palette: &ColorPalette) {
        ctx.set_active_console(HUD_CON);
        ctx.cls();
        let mut draw_batch = DrawBatch::new();
        draw_batch.fill_region(
            self.layout,
            ColorPair::new(palette.fg_hud, palette.bg_hud),
            to_cp437(' '),
        );
        draw_batch.draw_hollow_box(self.layout, ColorPair::new(palette.fg_hud, palette.bg_hud));
        // for (index, item) in self.items.iter().enumerate() {
        //     let color = if index == self.selection {
        //         ColorPair::new(palette.fg_hud, palette.bg_hud_selected)
        //     } else {
        //         ColorPair::new(palette.fg_hud, palette.bg_hud)
        //     };
        //     draw_batch.print_color(item.top_left_corner(), &item.text, color);
        // }

        // TODO: Render top row, middle row and bottom info field
        for item in &self.edit_functions {
            draw_batch.fill_region(
                item.layout,
                ColorPair::new(palette.fg_hud, palette.bg_hud_content),
                to_cp437(' '),
            );
            draw_batch.print_color(
                Point::new(item.layout.x1 + 1, item.layout.y1 + 1),
                item.idx.to_string(),
                ColorPair::new(palette.fg_hud_highlight, palette.bg_hud_content),
            );
            draw_batch.print_color(
                Point::new(item.layout.x1 + 3, item.layout.y1 + 1),
                item.title.to_string(),
                ColorPair::new(palette.fg_hud, palette.bg_hud_content),
            );
        }

        for item in &self.gene_items {
            let c: char = if modulus(item.gene_idx, 2) == 0 {
                '►'
            } else {
                '◄'
            };
            draw_batch.print_color(item.layout.center(), c, item.color);
        }

        draw_batch.submit(6000).unwrap();
    }
}
