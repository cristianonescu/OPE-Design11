//! Command layout of the other suite windows: the Programmable Stitch Creator and the Stitch
//! Design Factory (Application button menu, ribbon tabs and groups, Quick Access Toolbar) and
//! the Design Database (menu bar and toolbar). They follow the public online guide pages of the
//! embroidery suite OpenStitch is compatible with, as the Layout & Editing ribbon does
//! ([`super::ribbon`]); docs/UI_LAYOUT.md maps each command. Icons reuse OpenStitch's own
//! editor glyphs.
//!
//! Each window has its own command type implementing [`Command`]; groups are the editor's
//! [`Group`] over that type, so the binary draws every window with the same ribbon code.

use super::ribbon::{Action, Group, Info, Item};

/// How a command is drawn and run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Button,
    /// Opens a list of choices (dropdown arrow).
    Menu,
    /// On/off (drawn pressed while on); tools are toggles too.
    Toggle,
}

/// A command of one suite window.
pub trait Command: Copy + Eq + std::fmt::Debug + 'static {
    const ALL: &'static [Self];
    fn info(self) -> Info;
    fn kind(self) -> Kind;
    /// The editor command whose icon this one shows.
    fn icon(self) -> Action;
    fn is_menu(self) -> bool {
        self.kind() == Kind::Menu
    }
}

macro_rules! commands {
    ($(#[$doc:meta])* $name:ident {
        $($variant:ident => $key:literal, $label:literal, $tip:literal, $shortcut:expr, $engine:expr,
          $icon:ident, $kind:ident, $available:expr;)*
    }) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name { $($variant),* }

        impl Command for $name {
            const ALL: &'static [$name] = &[$($name::$variant),*];

            fn info(self) -> Info {
                match self {
                    $($name::$variant => Info {
                        key: $key,
                        label: $label,
                        tip: $tip,
                        shortcut: $shortcut,
                        engine: $engine,
                        available: $available,
                    }),*
                }
            }

            fn kind(self) -> Kind {
                match self { $($name::$variant => Kind::$kind),* }
            }

            fn icon(self) -> Action {
                match self { $($name::$variant => Action::$icon),* }
            }
        }
    };
}

/// A ribbon tab of a suite window.
#[derive(Clone, Copy, Debug)]
pub struct Tab<A: 'static> {
    pub key: &'static str,
    pub label: &'static str,
    pub groups: &'static [Group<A>],
}

/// A ribbon window: Application button menu, tabs, Quick Access Toolbar and the buttons at
/// the right of the tab row (Help, Options), drawn right to left.
#[derive(Clone, Copy, Debug)]
pub struct RibbonWindow<A: 'static> {
    pub title: &'static str,
    pub app_menu: &'static [A],
    pub tabs: &'static [Tab<A>],
    pub qat: &'static [A],
    pub buttons: &'static [A],
    /// At the bottom of the Application menu: back to Layout & Editing.
    pub exit: A,
}

impl<A: Command> RibbonWindow<A> {
    /// Every command the window shows, once.
    pub fn actions(&self) -> Vec<A> {
        let mut v = Vec::new();
        let tabs = self
            .tabs
            .iter()
            .flat_map(|t| t.groups.iter().flat_map(Group::actions));
        for a in self
            .app_menu
            .iter()
            .copied()
            .chain(tabs)
            .chain(self.qat.iter().copied())
            .chain(self.buttons.iter().copied())
            .chain([self.exit])
        {
            if !v.contains(&a) {
                v.push(a);
            }
        }
        v
    }

    pub fn tab(&self, key: &str) -> Option<usize> {
        self.tabs.iter().position(|t| t.key == key)
    }
}

/// A menu of a menu bar, in sections divided by separators.
#[derive(Clone, Copy, Debug)]
pub struct Menu<A: 'static> {
    pub name: &'static str,
    pub sections: &'static [&'static [A]],
}

/// A window with a classic menu bar and toolbar (the Design Database).
#[derive(Clone, Copy, Debug)]
pub struct MenuWindow<A: 'static> {
    pub title: &'static str,
    pub menus: &'static [Menu<A>],
    /// Toolbar buttons in sections.
    pub toolbar: &'static [&'static [A]],
}

impl<A: Command> MenuWindow<A> {
    pub fn actions(&self) -> Vec<A> {
        let mut v = Vec::new();
        let menus = self
            .menus
            .iter()
            .flat_map(|m| m.sections.iter().flat_map(|s| s.iter().copied()));
        for a in menus.chain(self.toolbar.iter().flat_map(|s| s.iter().copied())) {
            if !v.contains(&a) {
                v.push(a);
            }
        }
        v
    }
}

use Item::{Large, Small};

// MARK: Programmable Stitch Creator

commands! {
    /// Programmable Stitch Creator commands.
    CreatorAction {
        NewFill => "newFill", "New Fill/Stamp Pattern", "Start a new fill or stamp tile.", None, None, New, Button, true;
        NewMotif => "newMotif", "New Motif Pattern", "Start a new motif (a line from one end to the other).", None, None, New, Button, true;
        NewDecorative => "newDecorative", "New Decorative Fill Pattern", "Start a new decorative fill row.", None, None, New, Button, true;
        Open => "open", "Open", "Open an openstitch.pattern file to edit.", Some("Ctrl+O"), Some("pattern.import"), Open, Button, true;
        Save => "save", "Save", "Save the pattern in the user library folder.", Some("Ctrl+S"), None, Save, Button, true;
        SaveAs => "saveAs", "Save As", "Save the pattern as an openstitch.pattern file.", Some("Ctrl+Shift+S"), Some("pattern.export"), SaveAs, Button, true;
        Exit => "exit", "Exit", "Close the Programmable Stitch Creator and return to Layout & Editing.", None, None, Exit, Button, true;
        Help => "help", "Help", "Instruction manual and information about OpenStitch.", None, None, Help, Menu, true;
        SelectTool => "select", "Select Tool", "Click a line to make it the current one.", None, None, SelectTool, Toggle, true;
        EditPoint => "editPoint", "Edit Point Tool", "Drag points to move them; right-click deletes one.", None, None, PointTool, Toggle, true;
        DrawLine => "drawLine", "Draw Line Tool", "Click to add points to the current line.", None, None, OpenStraight, Toggle, true;
        NewLine => "newLine", "New Line", "Start another line in the tile (fill and decorative patterns).", None, None, OpenCurve, Button, true;
        RegionReset => "regionReset", "Region (Reset) Tool", "Clear the emboss or engrave effect of a region.", None, None, StampEmboss, Toggle, false;
        RegionEngrave => "regionEngrave", "Region (Engrave) Tool", "Mark a region to sew engraved.", None, None, StampEmboss, Toggle, false;
        RegionEmboss => "regionEmboss", "Region (Emboss) Tool", "Mark a region to sew embossed.", None, None, StampEmboss, Toggle, false;
        ZoomIn => "zoomIn", "Zoom In", "Magnify the Design Page.", Some("Ctrl+="), None, ZoomIn, Button, true;
        ZoomOut => "zoomOut", "Zoom Out", "Reduce the Design Page.", Some("Ctrl+-"), None, ZoomOut, Button, true;
        OtherZoom => "zoom", "Other Zoom Tools", "Zoom to fit, 200 % or 400 %.", None, None, OtherZoom, Menu, true;
        Pan => "pan", "Pan", "Drag the view.", None, None, Pan, Toggle, false;
        Cut => "cut", "Cut", "Copy the current line, then delete it.", Some("Ctrl+X"), None, Cut, Button, true;
        Copy => "copy", "Copy", "Copy the current line.", Some("Ctrl+C"), None, Copy, Button, true;
        Paste => "paste", "Paste", "Add the copied line to the pattern.", Some("Ctrl+V"), None, Paste, Button, true;
        Duplicate => "duplicate", "Duplicate", "Add a copy of the current line.", Some("Ctrl+D"), None, Duplicate, Button, true;
        Delete => "delete", "Delete", "Delete the current line (one always remains).", Some("Delete"), None, Delete, Button, true;
        FlipVertical => "flipVertical", "Flip vertically", "Mirror the current line top to bottom.", None, None, RotateFlip, Button, true;
        FlipHorizontal => "flipHorizontal", "Flip horizontally", "Mirror the current line left to right.", None, None, RotateFlip, Button, true;
        SplitAtPoint => "splitAtPoint", "Split at Point", "Split a line in two at the selected point.", None, None, SplitAtPoint, Button, false;
        AddToDesign => "addToDesign", "Add to Design", "Put the pattern in the open design (one Undo); choose it in the Sewing Attributes pane.", None, Some("pattern.define"), ConvertToStitches, Button, true;
        OpenTemplate => "openTemplate", "Open Template", "Show an image behind the pattern to trace.", None, None, OpenImage, Button, false;
        ModifyTemplate => "modifyTemplate", "Modify Template", "Move or scale the template image.", None, None, ModifyImage, Button, false;
        DeleteImage => "deleteImage", "Delete Image", "Remove the template image.", None, None, DeleteImage, Button, false;
        ImageDensity => "imageDensity", "Background Image Density", "Fade the template image.", None, None, ImageDensity, Menu, false;
        PreviewWindow => "previewWindow", "Show/Hide Preview window", "Show how the pattern sews in a region.", None, None, PaneSimulator, Toggle, true;
        StatusBar => "statusBar", "Show/Hide Status Bar", "Show the status bar.", None, None, PaneAttributes, Toggle, true;
        GridAxes => "gridAxes", "Show Grid with Axes", "Show the grid over the tile.", None, None, ShowGrid, Toggle, true;
        GridInterval => "gridInterval", "Grid interval", "Points snap to 1/16 of the tile.", None, None, GridSpacing, Menu, true;
        Guideline => "guideline", "Guideline", "Guidelines over the Design Page.", None, None, Guideline, Toggle, false;
    }
}

const CREATOR_HOME: &[Group<CreatorAction>] = {
    use CreatorAction as C;
    &[
        Group {
            name: "Select",
            items: &[Large(C::SelectTool), Small(&[C::EditPoint])],
        },
        Group {
            name: "Draw",
            items: &[
                Large(C::DrawLine),
                Small(&[C::NewLine]),
                Small(&[C::RegionReset, C::RegionEngrave, C::RegionEmboss]),
            ],
        },
        Group {
            name: "Zoom",
            items: &[
                Small(&[C::ZoomIn, C::ZoomOut, C::OtherZoom]),
                Small(&[C::Pan]),
            ],
        },
        Group {
            name: "Clipboard",
            items: &[
                Large(C::Paste),
                Small(&[C::Cut, C::Copy, C::Duplicate]),
                Small(&[C::Delete]),
            ],
        },
        Group {
            name: "Edit",
            items: &[Small(&[
                C::FlipVertical,
                C::FlipHorizontal,
                C::SplitAtPoint,
            ])],
        },
        Group {
            name: "Design",
            items: &[Large(C::AddToDesign)],
        },
    ]
};

const CREATOR_VIEW: &[Group<CreatorAction>] = {
    use CreatorAction as C;
    &[
        Group {
            name: "Template",
            items: &[
                Large(C::OpenTemplate),
                Small(&[C::ModifyTemplate, C::DeleteImage, C::ImageDensity]),
            ],
        },
        Group {
            name: "Show/Hide",
            items: &[Small(&[C::PreviewWindow, C::StatusBar])],
        },
        Group {
            name: "Grid",
            items: &[Large(C::GridAxes), Small(&[C::GridInterval])],
        },
        Group {
            name: "Guideline",
            items: &[Large(C::Guideline)],
        },
    ]
};

pub const CREATOR: RibbonWindow<CreatorAction> = {
    use CreatorAction as C;
    RibbonWindow {
        title: "Programmable Stitch Creator",
        app_menu: &[
            C::NewFill,
            C::NewMotif,
            C::NewDecorative,
            C::Open,
            C::Save,
            C::SaveAs,
        ],
        tabs: &[
            Tab {
                key: "home",
                label: "Home",
                groups: CREATOR_HOME,
            },
            Tab {
                key: "view",
                label: "View",
                groups: CREATOR_VIEW,
            },
        ],
        qat: &[C::Open, C::Save, C::AddToDesign],
        buttons: &[C::Help],
        exit: C::Exit,
    }
};

// MARK: Stitch Design Factory

commands! {
    /// Stitch Design Factory commands.
    FactoryAction {
        New => "new", "New", "Start a new sewing stitch pattern: decorative, satin, decorative satin or cross.", None, None, New, Menu, true;
        Open => "open", "Open", "Open an openstitch.sewing file to edit.", Some("Ctrl+O"), Some("sewing.import"), Open, Button, true;
        Save => "save", "Save", "Keep the pattern in the open project (one Undo).", Some("Ctrl+S"), Some("sewing.define"), Save, Button, true;
        SaveAs => "saveAs", "Save As", "Save the pattern as an openstitch.sewing file.", Some("Ctrl+Shift+S"), None, SaveAs, Button, true;
        Export => "export", "Export", "Write the pattern as openstitch.sewing JSON, SVG or PMV (experimental). Save it in the project first.", None, Some("sewing.export"), Export, Button, true;
        Exit => "exit", "Exit", "Close the Stitch Design Factory and return to Layout & Editing.", None, None, Exit, Button, true;
        Options => "options", "Options", "Colour theme.", None, None, Options, Menu, true;
        Help => "help", "Help", "Instruction manual and information about OpenStitch.", None, None, Help, Menu, true;
        SelectObject => "selectObject", "Select Object", "Select whole stitch objects.", None, None, SelectTool, Toggle, false;
        SelectPoint => "selectPoint", "Select Point", "Drag points to move them; right-click deletes one.", None, None, PointTool, Toggle, true;
        LineTool => "line", "Line Tool", "Click to add needle points, satin stations, outline points or cross cells.", None, None, OpenStraight, Toggle, true;
        PenTool => "pen", "Pen Tool", "Draw curves that are sewn as stitches.", None, None, ClosedCurve, Toggle, false;
        ZoomIn => "zoomIn", "Zoom in", "Magnify the design area.", Some("Ctrl+="), None, ZoomIn, Button, true;
        ZoomOut => "zoomOut", "Zoom Out", "Reduce the design area.", Some("Ctrl+-"), None, ZoomOut, Button, true;
        SelectedZoom => "selectedZoom", "Selected object zoom", "Zoom to the selected object.", None, None, OtherZoom, Button, false;
        Pan => "pan", "Pan", "Drag the view.", None, None, Pan, Toggle, false;
        SingleStitch => "single", "Single Stitch", "Cross stitches sew each diagonal once.", None, None, OpenStraight, Toggle, true;
        TripleStitch => "triple", "Triple Stitch", "Cross stitches sew each diagonal three times.", None, None, Running, Toggle, true;
        Swap => "swap", "Swap", "Swap the start and end of the selected stitches.", None, None, Connect, Button, false;
        FlipVertical => "flipVertical", "Flip Vertical", "Mirror the pattern across the feed.", None, None, RotateFlip, Button, true;
        FlipHorizontal => "flipHorizontal", "Flip Horizontal", "Mirror the pattern along the feed.", None, None, RotateFlip, Button, true;
        Duplicate => "duplicate", "Duplicate", "Add a copy of the selected stitches.", Some("Ctrl+D"), None, Duplicate, Button, false;
        Delete => "delete", "Delete", "Delete the selected point (each kind keeps its minimum).", Some("Delete"), None, Delete, Button, true;
        OpenTemplate => "openTemplate", "Open Template", "Show an image behind the pattern to trace.", None, None, OpenImage, Button, false;
        ModifyTemplate => "modifyTemplate", "Modify Template", "Move or scale the template image.", None, None, ModifyImage, Button, false;
        DeleteImage => "deleteImage", "Delete Image", "Remove the template image.", None, None, DeleteImage, Button, false;
        TemplateDensity => "templateDensity", "Template Density", "Fade the template image.", None, None, ImageDensity, Menu, false;
        NeedleDropPoint => "needleDrops", "Needle Drop Point", "Show the needle drop points.", None, None, SewingPoints, Toggle, true;
        HorizontalOrientation => "horizontal", "Horizontal Orientation", "Turn the design area to sew upwards instead of to the right.", None, None, ImageRotateRight, Toggle, false;
        ImportPane => "importPane", "Import Pane", "The patterns kept in the project; Edit loads one.", None, Some("sewing.list"), PaneImport, Toggle, true;
        PreviewWindow => "previewWindow", "Preview Window", "The pattern repeated as the machine sews it.", None, None, PaneSimulator, Toggle, true;
        StatusBar => "statusBar", "Status Bar", "Show the status bar.", None, None, PaneAttributes, Toggle, true;
        ShowGrid => "showGrid", "Show Grid", "Show a 1 mm grid.", None, None, ShowGrid, Toggle, true;
        SnapToGrid => "snapToGrid", "Snap to Grid", "Points snap to 0.25 mm.", None, None, SnapToGrid, Toggle, true;
        GridSpacing => "gridSpacing", "Grid Spacing", "Distance between grid lines.", None, None, GridSpacing, Menu, false;
        StitchSimulator => "simulator", "Stitch Simulator", "Play the stitching needle by needle.", None, None, PaneSimulator, Toggle, false;
    }
}

const FACTORY_HOME: &[Group<FactoryAction>] = {
    use FactoryAction as F;
    &[
        Group {
            name: "Select",
            items: &[Small(&[F::SelectObject, F::SelectPoint])],
        },
        Group {
            name: "Draw",
            items: &[Large(F::LineTool), Large(F::PenTool)],
        },
        Group {
            name: "Zoom",
            items: &[
                Small(&[F::ZoomIn, F::ZoomOut, F::SelectedZoom]),
                Small(&[F::Pan]),
            ],
        },
        Group {
            name: "Stitch",
            items: &[Small(&[F::SingleStitch, F::TripleStitch, F::Swap])],
        },
        Group {
            name: "Edit",
            items: &[
                Small(&[F::FlipVertical, F::FlipHorizontal]),
                Small(&[F::Duplicate, F::Delete]),
            ],
        },
    ]
};

const FACTORY_VIEW: &[Group<FactoryAction>] = {
    use FactoryAction as F;
    &[
        Group {
            name: "Template",
            items: &[
                Large(F::OpenTemplate),
                Small(&[F::ModifyTemplate, F::DeleteImage, F::TemplateDensity]),
            ],
        },
        Group {
            name: "Display",
            items: &[Small(&[F::NeedleDropPoint, F::HorizontalOrientation])],
        },
        Group {
            name: "Show/Hide",
            items: &[Small(&[F::ImportPane, F::PreviewWindow, F::StatusBar])],
        },
        Group {
            name: "Grid",
            items: &[Large(F::ShowGrid), Small(&[F::SnapToGrid, F::GridSpacing])],
        },
        Group {
            name: "Simulator",
            items: &[Large(F::StitchSimulator)],
        },
    ]
};

pub const FACTORY: RibbonWindow<FactoryAction> = {
    use FactoryAction as F;
    RibbonWindow {
        title: "Stitch Design Factory",
        app_menu: &[F::New, F::Open, F::Save, F::SaveAs, F::Export],
        tabs: &[
            Tab {
                key: "home",
                label: "Home",
                groups: FACTORY_HOME,
            },
            Tab {
                key: "view",
                label: "View",
                groups: FACTORY_VIEW,
            },
        ],
        qat: &[F::Open, F::Save, F::Export],
        buttons: &[F::Help, F::Options],
        exit: F::Exit,
    }
};

// MARK: Design Database

commands! {
    /// Design Database commands (menu bar and toolbar).
    DbAction {
        OpenInEditor => "openInEditor", "Open in Layout & Editing", "Open the selected design as the editor's design.", Some("Enter"), Some("project.open"), Open, Button, true;
        ImportIntoEditor => "importIntoEditor", "Import Into Layout & Editing", "Add the selected designs to the editor's design.", None, Some("stitches.import"), ImportPatterns, Button, true;
        NewFolder => "newFolder", "Create New Folder", "Create a folder in the current folder.", None, None, Open, Button, true;
        ConvertFormat => "convertFormat", "Convert Format", "Write the selected designs in another embroidery format.", None, Some("library.convert"), Export, Button, true;
        Delete => "delete", "Delete", "Move the selected files to the Trash.", Some("Delete"), None, Delete, Button, true;
        Rename => "rename", "Rename", "Rename the selected file (the extension stays).", Some("F2"), None, EditText, Button, true;
        Search => "search", "Search", "Find designs by name, format, size, stitches, colours, date or PES properties.", None, Some("library.search"), OtherZoom, Button, true;
        PrintSetup => "printSetup", "Print Setup", "Paper, layout and colour list of the printed catalogue; print or save it as a PDF.", None, None, DesignSettings, Button, true;
        PrintPreview => "printPreview", "Print Preview", "Catalogue pages as images to check before printing.", None, None, Print, Button, true;
        Print => "print", "Print", "Print the catalogue with the system print dialog, using the Print Setup options.", None, Some("library.catalog"), Print, Button, true;
        ImageHtml => "imageHtml", "Create Image File and HTML", "A web page with an image of each design.", None, Some("library.catalog"), OpenImage, Button, true;
        Csv => "csv", "Create CSV", "A spreadsheet of the designs and their threads.", None, Some("library.catalog"), Export, Button, true;
        Exit => "exit", "Exit", "Close the Design Database and return to Layout & Editing.", None, None, Exit, Button, true;
        Cut => "cut", "Cut", "Move the selected files with the next Paste.", Some("Ctrl+X"), None, Cut, Button, true;
        Copy => "copy", "Copy", "Copy the selected files with the next Paste.", Some("Ctrl+C"), None, Copy, Button, true;
        Paste => "paste", "Paste", "Put the cut or copied files in the current folder.", Some("Ctrl+V"), None, Paste, Button, true;
        SelectAll => "selectAll", "Select All", "Select every design in the folder.", Some("Ctrl+A"), None, SelectAll, Button, true;
        LargeThumbnails => "large", "Large Thumbnails", "Show the designs as large thumbnails.", None, None, ShapesTools, Toggle, true;
        SmallThumbnails => "small", "Small Thumbnails", "Show the designs as small thumbnails.", None, None, ShowGrid, Toggle, true;
        Details => "details", "Details", "Show the designs as a list with their sewing information.", None, None, PaneSewingOrder, Toggle, true;
        Preview => "preview", "Preview", "Show the selected design larger.", None, None, PaneSimulator, Toggle, true;
        Property => "property", "Property", "Show the file information of the selected design.", None, None, DesignProperty, Toggle, true;
        Refresh => "refresh", "Refresh", "Read the folder again.", None, None, Redo, Button, true;
        Toolbar => "toolbar", "Toolbar", "Show or hide the toolbar.", None, None, PaneAttributes, Toggle, true;
        StatusBar => "statusBar", "Status Bar", "Show or hide the status bar.", None, None, PaneAttributes, Toggle, true;
        SystemUnit => "systemUnit", "Select System Unit", "Sizes in mm or inches.", None, None, Ruler, Menu, true;
        ApplicationColor => "appColor", "Application Color", "Colour theme of OpenStitch.", None, None, SelectColorPalette, Menu, true;
        UsbLink => "usbLink", "USB Link", "Embroider from the computer over a USB cable.", None, None, SendEmbroidery, Button, false;
        WirelessLink => "wirelessLink", "Wireless Link", "Embroider from the computer over the network.", None, None, SendEmbroidery, Button, false;
        DstSettings => "dstSettings", "DST Settings", "Jumps that make a trim when writing .dst.", None, None, DesignSettings, Menu, true;
        NetworkMachines => "networkMachines", "Network Machine Settings", "Register machines on the wireless network.", None, None, DesignSettings, Button, false;
        Manual => "manual", "Online Instruction Manual", "OpenStitch's documentation.", None, None, Help, Button, true;
        About => "about", "About Design Database", "Version and licence of OpenStitch.", None, None, DesignProperty, Button, true;
        Support => "support", "Customer Support", "Report a problem or ask a question.", None, None, Help, Button, true;
        ParentFolder => "parent", "Parent Folder", "Go to the folder that holds this one.", Some("Alt+Up"), None, Undo, Button, true;
        ChooseFolder => "chooseFolder", "Choose Folder", "Browse to a folder.", None, None, Open, Button, true;
        ReadCard => "readCard", "Card Reading", "Read the designs on an original card. Blocked: Brother has not published the USB card reader/writer protocol or the card data layout, and OpenStitch will not reverse-engineer them.", None, None, ImportPatterns, Button, false;
        WritingBox => "writingBox", "Writing List", "Show the list of designs to write to USB media (Add, Remove, Write).", None, None, SendEmbroidery, Toggle, true;
    }
}

pub const DATABASE: MenuWindow<DbAction> = {
    use DbAction as D;
    MenuWindow {
        title: "Design Database",
        menus: &[
            Menu {
                name: "File",
                sections: &[
                    &[D::OpenInEditor, D::ImportIntoEditor],
                    &[
                        D::NewFolder,
                        D::ConvertFormat,
                        D::Delete,
                        D::Rename,
                        D::Search,
                    ],
                    &[D::PrintSetup, D::PrintPreview, D::Print],
                    &[D::ImageHtml, D::Csv],
                    &[D::Exit],
                ],
            },
            Menu {
                name: "Edit",
                sections: &[&[D::Cut, D::Copy, D::Paste], &[D::SelectAll]],
            },
            Menu {
                name: "Display",
                sections: &[
                    &[D::LargeThumbnails, D::SmallThumbnails, D::Details],
                    &[D::Preview, D::Property],
                    &[D::Refresh],
                    &[D::Toolbar, D::StatusBar],
                ],
            },
            Menu {
                name: "Option",
                sections: &[
                    &[D::SystemUnit, D::ApplicationColor],
                    &[D::UsbLink, D::WirelessLink],
                    &[D::DstSettings, D::NetworkMachines],
                ],
            },
            Menu {
                name: "Help",
                sections: &[&[D::Manual, D::About], &[D::Support]],
            },
        ],
        toolbar: &[
            &[D::Exit, D::ParentFolder, D::ChooseFolder],
            &[D::Cut, D::Copy, D::Paste],
            &[D::LargeThumbnails, D::SmallThumbnails, D::Details],
            &[D::Preview, D::Property],
            &[
                D::Search,
                D::ConvertFormat,
                D::OpenInEditor,
                D::ImportIntoEditor,
            ],
            &[D::ReadCard, D::WritingBox],
        ],
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    fn check<A: Command>(all: &[A], shown: &[A]) {
        let mut keys: Vec<&str> = all.iter().map(|a| a.info().key).collect();
        keys.sort_unstable();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n, "duplicate key");
        for &a in all {
            assert!(shown.contains(&a), "{a:?} is in no menu, tab or toolbar");
            let info = a.info();
            assert!(!info.label.is_empty() && !info.tip.is_empty(), "{a:?}");
            let tip = info.screen_tip(false);
            assert_eq!(tip.contains("Not yet available"), !info.available, "{a:?}");
            if let Some(s) = info.shortcut {
                assert!(tip.contains(s), "{a:?}");
            }
        }
    }

    fn groups_hold_three_per_column<A: Command>(w: &RibbonWindow<A>) {
        for t in w.tabs {
            assert!(!t.groups.is_empty());
            assert_eq!(w.tab(t.key).map(|i| w.tabs[i].label), Some(t.label));
            for g in t.groups {
                assert!(!g.name.is_empty());
                for i in g.items {
                    if let Item::Small(a) = i {
                        assert!((1..=3).contains(&a.len()), "{} {}", t.label, g.name);
                    }
                }
            }
        }
    }

    #[test]
    fn every_suite_command_has_a_place_and_a_screen_tip() {
        check(CreatorAction::ALL, &CREATOR.actions());
        check(FactoryAction::ALL, &FACTORY.actions());
        check(DbAction::ALL, &DATABASE.actions());
        groups_hold_three_per_column(&CREATOR);
        groups_hold_three_per_column(&FACTORY);
    }

    #[test]
    fn layouts_follow_the_guide() {
        let labels = |a: &[CreatorAction]| a.iter().map(|a| a.info().label).collect::<Vec<_>>();
        assert_eq!(
            labels(CREATOR.app_menu),
            [
                "New Fill/Stamp Pattern",
                "New Motif Pattern",
                "New Decorative Fill Pattern",
                "Open",
                "Save",
                "Save As"
            ]
        );
        let f: Vec<_> = FACTORY.app_menu.iter().map(|a| a.info().label).collect();
        assert_eq!(f, ["New", "Open", "Save", "Save As", "Export"]);
        for w in [CREATOR.tabs.len(), FACTORY.tabs.len()] {
            assert_eq!(w, 2, "Home and View");
        }
        let menus: Vec<_> = DATABASE.menus.iter().map(|m| m.name).collect();
        assert_eq!(menus, ["File", "Edit", "Display", "Option", "Help"]);
    }

    #[test]
    fn every_engine_command_named_exists() {
        let session = openstitch_engine::Session::new(std::env::temp_dir());
        let ids: Vec<&str> = session.commands().iter().map(|c| c.id).collect();
        let engines = CreatorAction::ALL
            .iter()
            .map(|a| a.info())
            .chain(FactoryAction::ALL.iter().map(|a| a.info()))
            .chain(DbAction::ALL.iter().map(|a| a.info()));
        for info in engines {
            if let Some(e) = info.engine {
                assert!(ids.contains(&e), "{} names unknown command {e}", info.label);
            }
        }
    }
}
