//! The editor's command layout: every command the window offers as an [`Action`], the ribbon
//! tabs and groups that show them, the contextual tabs that appear for the active tool or the
//! selection, the Application menu and the Quick Access Toolbar. The arrangement and command
//! names follow the public online guide of the embroidery suite OpenStitch is compatible with
//! (docs/UI_LAYOUT.md maps each one), so its users find commands where they expect them; icons
//! and visual style are OpenStitch's own.
//!
//! The egui shell (`ribbon.rs` in the binary) draws this model and runs the actions; nothing
//! here depends on egui, so the layout rules are unit-tested.

use super::tools::Tool;

/// What one command shows: label, ScreenTip, shortcut, the engine command it runs (when it maps
/// to one), and whether OpenStitch implements it yet (`false` = shown disabled with a
/// "not yet available" ScreenTip).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Info {
    /// Stable name used in the user's settings (Quick Access Toolbar).
    pub key: &'static str,
    pub label: &'static str,
    pub tip: &'static str,
    /// As written on Windows and Linux ("Ctrl+S"); see [`Info::shortcut_for`].
    pub shortcut: Option<&'static str>,
    pub engine: Option<&'static str>,
    pub available: bool,
}

impl Info {
    /// The shortcut as shown on this platform (Cmd on macOS).
    pub fn shortcut_for(&self, mac: bool) -> Option<String> {
        self.shortcut.map(|s| {
            if mac {
                s.replace("Ctrl+", "Cmd+")
            } else {
                s.to_string()
            }
        })
    }

    /// ScreenTip text: the label, the shortcut and what the command does.
    pub fn screen_tip(&self, mac: bool) -> String {
        let head = match self.shortcut_for(mac) {
            Some(s) => format!("{} ({s})", self.label),
            None => self.label.to_string(),
        };
        if self.available {
            format!("{head}\n{}", self.tip)
        } else {
            format!("{head}\nNot yet available in OpenStitch. {}", self.tip)
        }
    }
}

macro_rules! actions {
    ($($variant:ident => $key:literal, $label:literal, $tip:literal, $shortcut:expr, $engine:expr, $available:expr;)*) => {
        /// Every command of the Layout & Editing window.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Action { $($variant),* }

        impl Action {
            pub const ALL: &'static [Action] = &[$(Action::$variant),*];

            pub fn info(self) -> Info {
                match self {
                    $(Action::$variant => Info {
                        key: $key,
                        label: $label,
                        tip: $tip,
                        shortcut: $shortcut,
                        engine: $engine,
                        available: $available,
                    }),*
                }
            }
        }
    };
}

actions! {
    // Application button menu.
    New => "new", "New", "Start a new design.", Some("Ctrl+N"), Some("design.new"), true;
    Open => "open", "Open", "Open an OpenStitch design, or an embroidery file as a new design.", Some("Ctrl+O"), Some("project.open"), true;
    Save => "save", "Save", "Save the design.", Some("Ctrl+S"), Some("project.save"), true;
    SaveAs => "saveAs", "Save As", "Save the design under a new name.", Some("Ctrl+Shift+S"), Some("project.save"), true;
    Print => "print", "Print", "Print the design to a PDF with its colour list.", None, Some("design.print"), true;
    DesignProperty => "designProperty", "Design Property", "Sewing information of the design: size, stitches, colours, thread and time.", None, Some("design.info"), true;
    DesignSettings => "designSettings", "Design Settings", "Machine, hoop and Design Page size.", None, Some("machine.set"), true;
    FabricSelector => "fabricSelector", "Fabric Selector", "Suggested sewing attributes for a fabric type.", None, None, false;
    SelectColorPalette => "colorPalette", "Select Color Palette", "Choose or edit the thread colour charts.", None, Some("threads.charts"), true;
    Wizard => "wizard", "Wizard", "Create a design step by step: templates or an image.", None, None, true;
    Export => "export", "Export", "Write the design as a machine embroidery file.", None, Some("design.export"), true;
    Options => "options", "Options", "Other applications, settings and the colour theme.", None, None, true;
    Exit => "exit", "Exit", "Close OpenStitch.", None, None, true;
    // Quick Access Toolbar extras.
    Undo => "undo", "Undo", "Undo the last change.", Some("Ctrl+Z"), Some("history.undo"), true;
    Redo => "redo", "Redo", "Redo the change undone last.", Some("Ctrl+Y"), Some("history.redo"), true;
    Help => "help", "Help", "Instruction manual, keyboard shortcuts and information about OpenStitch.", None, None, true;
    // Option button.
    DesignDatabase => "designDatabase", "Design Database", "Browse, search and convert embroidery files in folders.", None, Some("library.list"), true;
    StitchCreator => "stitchCreator", "Programmable Stitch Creator", "Edit fill, stamp and motif stitch patterns.", None, Some("pattern.define"), true;
    StitchDesignFactory => "stitchFactory", "Stitch Design Factory", "Edit decorative stitches sewn along lines.", None, Some("sewing.define"), true;
    FontCreator => "fontCreator", "Font Creator", "Create embroidery fonts and map characters.", None, Some("fonts.create"), true;
    AttributeSets => "attributeSets", "Sewing Attribute Sets", "Save and apply named sets of sewing attributes.", None, Some("attributes.save"), true;
    UserThreadCharts => "threadCharts", "User Thread Charts", "Make your own thread colour charts.", None, Some("threads.createChart"), true;
    // Home ▸ Select.
    SelectTool => "select", "Select", "Select objects; drag to move, handles scale and rotate.", Some("V"), None, true;
    PointTool => "point", "Point Edit", "Move, add and delete points of the selected outline.", Some("A"), Some("object.setPoint"), true;
    SelectAll => "selectAll", "Select All", "Select every visible object.", Some("Ctrl+A"), None, true;
    // Home ▸ Clipboard.
    Cut => "cut", "Cut", "Move the selection to the clipboard. Use Copy, then Delete.", Some("Ctrl+X"), None, false;
    Copy => "copy", "Copy", "Copy the selection; Paste adds a copy of it.", Some("Ctrl+C"), None, true;
    Paste => "paste", "Paste", "Add a copy of the copied objects.", Some("Ctrl+V"), Some("object.duplicate"), true;
    Duplicate => "duplicate", "Duplicate", "Add a copy of the selection, slightly offset.", Some("Ctrl+D"), Some("object.duplicate"), true;
    Delete => "delete", "Delete", "Delete the selection.", Some("Delete"), Some("object.delete"), true;
    ArrangeCopy => "arrangeCopy", "Arrange Copy", "Copies in a row, a grid or around a circle.", None, Some("object.arrangeCopies"), true;
    CopyAttributes => "copyAttributes", "Copy Attributes", "Remember the sewing attributes of the selected object.", None, None, true;
    PasteAttributes => "pasteAttributes", "Paste Attributes", "Give the selection the copied sewing attributes.", None, Some("object.pasteAttributes"), true;
    // Home ▸ Tools.
    TextTools => "text", "Text", "Text, monograms, name drop lists, templates and fonts.", None, Some("text.add"), true;
    ShapesTools => "shapes", "Shapes", "Draw rectangles, ellipses, outlines and curves, or insert preset shapes.", None, Some("shape.preset"), true;
    Rectangle => "rectangle", "Rectangle", "Drag to draw a rectangle.", Some("R"), Some("shape.rectangle"), true;
    Ellipse => "ellipse", "Circle / Ellipse", "Drag to draw an ellipse.", Some("E"), Some("shape.ellipse"), true;
    ClosedStraight => "polygon", "Closed Straight Line", "Click the corners of a closed outline.", Some("P"), Some("shape.polygon"), true;
    ClosedCurve => "pen", "Closed Curve", "Click for corners, drag for curves; click the first point to close.", Some("B"), Some("shape.path"), true;
    PencilClosed => "pencilClosed", "Pencil (Closed Curve)", "Draw a closed outline freehand.", None, None, false;
    OpenStraight => "openStraight", "Open Straight Line", "Click the points of an open line.", None, None, false;
    OpenCurve => "openCurve", "Open Curve", "Pen tool: Return finishes the curve open.", None, Some("shape.path"), true;
    PencilOpen => "pencilOpen", "Pencil (Open Curve)", "Draw an open line freehand.", None, None, false;
    ZoomIn => "zoomIn", "Zoom In", "Magnify the view.", Some("Ctrl+="), None, true;
    ZoomOut => "zoomOut", "Zoom Out", "Reduce the view.", Some("Ctrl+-"), None, true;
    OtherZoom => "zoom", "Zoom", "Fit the Design Page, the selection, or 100 %.", Some("Ctrl+0"), None, true;
    Pan => "pan", "Pan", "Drag the view (also Space+drag or the middle button).", Some("H"), None, true;
    Measure => "measure", "Measure", "Measure a distance on the Design Page.", Some("M"), Some("design.measure"), true;
    // Home ▸ Import.
    ImportPatterns => "import", "Import Patterns", "Add an embroidery file to the design.", Some("Ctrl+Shift+I"), Some("stitches.import"), true;
    ImportSvg => "importSvg", "Import SVG", "Add vector artwork as editable shapes.", None, Some("import.svg"), true;
    // Home ▸ Edit.
    Arrange => "arrange", "Arrange", "Centre the selection or change where it sews in the order.", None, Some("order.move"), true;
    RotateFlip => "rotateFlip", "Rotate/Flip", "Rotate by 90° or mirror the selection.", None, Some("object.rotate"), true;
    Size => "size", "Size", "Scale the selection by a percentage.", None, Some("object.scale"), true;
    Group => "group", "Group", "Group the selection so it moves together.", Some("Ctrl+G"), Some("object.group"), true;
    Ungroup => "ungroup", "Ungroup", "Take the selected groups apart.", Some("Ctrl+Shift+G"), Some("object.ungroup"), true;
    StampEmboss => "stampEmboss", "Stamp/Emboss", "Emboss or engrave a shape, or place stamps in it.", None, Some("shape.emboss"), true;
    ModifyOverlap => "modifyOverlap", "Modify Overlap", "Merge, remove overlap or combine outlines.", None, Some("shape.combine"), true;
    DecorativeFrame => "decorativeFrame", "Decorative Frame", "Add a frame around the selection.", None, Some("shape.decorative"), true;
    FitTextToOutline => "fitText", "Fit Text to outline", "Select a text and a shape: the text follows the shape's outline.", None, Some("text.setPath"), true;
    // Home ▸ Wizards.
    BackgroundFill => "backgroundFill", "Background fill", "Fill around the selection with a background stitch.", None, Some("wizard.backgroundFill"), true;
    Applique => "applique", "Applique Wizard", "Placement, tack-down and cover stitching for an appliqué.", None, Some("wizard.applique"), true;
    Patch => "patch", "Embroidered Patch Wizard", "A patch with a satin border around the selection.", None, Some("wizard.patch"), true;
    Cutwork => "cutwork", "Cutwork Wizard", "Cut lines and the stitching around them.", None, Some("wizard.cutwork"), true;
    // Home ▸ Sew.
    SendEmbroidery => "send", "Sew Embroidery", "Write the design to USB media, or each hooping as its own file.", None, Some("transfer.send"), true;
    // Image tab.
    OpenImage => "openImage", "Open Image", "Show an image behind the design (PNG, JPEG, BMP).", None, Some("background.set"), true;
    PasteImage => "pasteImage", "Paste Image", "Use the image on the clipboard as the background.", None, Some("background.set"), true;
    SaveImage => "saveImage", "Save Image", "Save the background image to a file.", None, None, false;
    ModifyImage => "modifyImage", "Modify Image", "Crop or adjust the background image.", None, None, false;
    ImageDensity => "imageDensity", "Background Image Density", "Show, hide or fade the background image.", None, Some("background.show"), true;
    ImageRotateRight => "imageRotateRight", "Rotate Right", "Turn the background image 90° clockwise.", None, None, false;
    ImageRotateLeft => "imageRotateLeft", "Rotate Left", "Turn the background image 90° anticlockwise.", None, None, false;
    DeleteImage => "deleteImage", "Delete Image", "Remove the background image.", None, Some("background.clear"), true;
    AutoPunch => "autoPunch", "Auto Punch Wizard", "Regions and outlines traced from an image.", None, Some("digitize.autoPunch"), true;
    CrossStitch => "crossStitch", "Cross Stitch Wizard", "Cross stitches in the image's colours.", None, Some("digitize.crossStitch"), true;
    PhotoStitch1 => "photoStitch1", "Photo Stitch 1 Wizard", "A photo as fill stitching.", None, Some("digitize.photoStitch"), true;
    PhotoStitch2 => "photoStitch2", "Photo Stitch 2 Wizard", "A photo as zigzag lines.", None, Some("digitize.lineImage"), true;
    DesignCenter => "designCenter", "Import to Design Center", "Trace a line drawing into outlines and regions.", None, Some("digitize.designCenter"), true;
    // View tab.
    SolidView => "solidView", "Solid View", "Stitches drawn as solid colour areas.", None, None, false;
    StitchView => "stitchView", "Stitch View", "Stitches drawn as lines.", None, None, true;
    RealisticView => "realisticView", "Realistic View", "Stitches drawn as thread with shading.", None, None, true;
    ViewThreadTrimming => "viewTrimming", "View Thread Trimming", "Show the jumps between stitched areas.", None, None, true;
    RealisticSettings => "realisticSettings", "Realistic View Settings", "Brightness and contrast of the realistic view.", None, None, false;
    PaneSewingOrder => "paneSewingOrder", "Sewing Order", "Show or hide the Sewing Order pane.", None, None, true;
    PaneSimulator => "paneSimulator", "Stitch Simulator", "Show or hide the Stitch Simulator pane.", None, None, true;
    PaneAttributes => "paneAttributes", "Attributes Setting", "Show or hide the Color / Sewing Attributes / Text Attributes pane.", None, None, true;
    PaneImport => "paneImport", "Import Pane", "Show the Import pane.", None, None, true;
    ReferenceWindow => "reference", "Reference Window", "An overview of the Design Page; click to move the view.", None, None, true;
    ShowGrid => "showGrid", "Show Grid", "Show the grid on the Design Page.", None, None, true;
    GridAxes => "gridAxes", "Show Grid with Axes", "Grid with axes through the centre of the Design Page.", None, None, false;
    SnapToGrid => "snapToGrid", "Snap to Grid", "Objects move and draw in grid steps.", None, None, true;
    GridSpacing => "gridSpacing", "Grid Spacing", "Distance between grid lines.", None, None, true;
    Ruler => "ruler", "Ruler", "Show rulers along the work area.", None, None, true;
    Guideline => "guideline", "Guideline", "Guidelines dragged out of the rulers.", None, None, false;
    // Contextual tabs.
    Font => "font", "Font", "Font of the selected text.", None, Some("object.setText"), true;
    TextSize => "textSize", "Text Size", "Size of the selected text.", None, Some("object.setText"), true;
    FontFilter => "fontFilter", "Font Filter", "Show only fonts of a kind.", None, None, false;
    FontFilterSettings => "fontFilterSettings", "Font Filter Settings", "Choose the font kinds the filter shows.", None, None, false;
    TrueTypeAttributes => "trueType", "TrueType Font Attribute Setting", "Load a TrueType or OpenType font into the design.", None, Some("fonts.loadTrueType"), true;
    EditText => "editText", "Edit Text", "Change the characters of the selected text.", None, Some("object.setText"), true;
    LineColor => "lineColor", "Line color", "Thread colour of the outline.", None, Some("object.setLine"), true;
    LineSewType => "lineSewType", "Line sew type", "How the outline is sewn (running, triple, zigzag, motif…).", None, Some("object.setLine"), true;
    RegionColor => "regionColor", "Region color", "Thread colour of the region.", None, Some("object.setRegion"), true;
    RegionSewType => "regionSewType", "Region sew type", "How the region is sewn (fill, satin, programmable…).", None, Some("object.setRegion"), true;
    ConvertToBlocks => "toBlocks", "Convert to Blocks", "Turn the stitch pattern into editable regions.", None, Some("object.toShapes"), true;
    ConvertToStitches => "toStitches", "Convert to Stitches", "Turn the selection into a stitch pattern.", None, Some("object.toStitches"), true;
    ConvertToOutline => "toOutline", "Convert to Outline", "Text: one shape per character. Stitches: their outline.", None, Some("object.toShapes"), true;
    EdgeRadius => "edgeRadius", "Edge radius", "Round the corners of the rectangle.", None, None, false;
    StraightBlock => "punch", "Straight Block", "Click both ends of each sewing-direction line.", Some("U"), Some("shape.column"), true;
    CurvedBlock => "curvedBlock", "Curved Block", "Blocks with curved sides.", None, None, false;
    Running => "running", "Running", "Running stitch between clicked points.", None, None, false;
    Feed => "feed", "Feed", "Jump between clicked points.", None, None, false;
    SplitOutline => "splitOutline", "Split Outline", "Draw a cut line that splits the selected shape.", None, Some("shape.split"), true;
    AddDirectionLine => "addDirection", "Add Direction Line", "Draw a curved sewing direction across the region.", None, Some("object.setDirectionCurve"), true;
    DeleteDirectionLine => "deleteDirection", "Delete Direction Line", "Back to a straight sewing direction.", None, Some("object.setDirectionLines"), true;
    ReverseDirectionLine => "reverseDirection", "Reverse Direction Line", "Sew the direction line the other way.", None, None, false;
    EditPoint => "editPoint", "Edit Point", "Move the points of the outline.", Some("A"), Some("object.setPoint"), true;
    DeletePoint => "deletePoint", "Delete Point", "Delete the selected point.", Some("Delete"), Some("object.deletePoint"), true;
    SplitAtPoint => "splitAtPoint", "Split at Point", "Split the stitch pattern at the selected needle point.", None, Some("stitches.splitAt"), true;
    ConvertRegionToLine => "regionToLine", "Convert Region to Line", "Sew the outline with a running stitch in the region's colour.", None, Some("object.regionToLine"), true;
    OffsetLines => "offsetLines", "Create Offset Lines", "Lines inside or outside the outline.", None, Some("shape.offsetLines"), true;
    FloralPattern => "floral", "Create Floral Pattern", "A floral pattern inside the outline.", None, Some("shape.floral"), true;
    EntryExit => "entryExit", "Entry/Exit Points", "Where the region and line start and end.", None, Some("object.setEntryExit"), true;
    SewingPoints => "sewingPoints", "Center/Reference Point", "Centre point of radial fills and the reference point of stitch patterns.", None, Some("object.setCenterPoint"), true;
    SplitStitchData => "splitStitchData", "Split Stitch Data", "Draw around stitches to split them off.", None, Some("stitches.splitArea"), true;
    ChangeColor => "changeColor", "Change Color", "Change a thread colour of the stitch pattern.", None, Some("threads.remap"), false;
    DivideByColor => "divideByColor", "Divide by Color", "One stitch pattern per thread colour.", None, Some("stitches.divideByColor"), true;
    Connect => "connect", "Connect", "The pattern sewn later starts where the earlier one ends.", None, Some("stitches.connect"), true;
    ConvertToOutlines => "toOutlines", "Convert to Outlines", "One outline per thread colour of the stitches.", None, Some("object.toShapes"), true;
    MoveSections => "moveSections", "Move Sections", "Move the hooping sections of a large design.", None, None, false;
    ExportSections => "exportSections", "Export Sections", "One file per hooping of a design larger than the hoop.", None, Some("sections.export"), true;
}

impl Action {
    pub fn from_key(key: &str) -> Option<Action> {
        Action::ALL.iter().copied().find(|a| a.info().key == key)
    }

    /// Commands that open a list of choices (drawn with a dropdown arrow).
    pub fn is_menu(self) -> bool {
        use Action::*;
        matches!(
            self,
            Print
                | Wizard
                | Export
                | Help
                | Options
                | TextTools
                | ShapesTools
                | OtherZoom
                | Arrange
                | RotateFlip
                | Size
                | StampEmboss
                | ModifyOverlap
                | DecorativeFrame
                | SendEmbroidery
                | ImageDensity
                | GridSpacing
                | Font
                | TextSize
                | EditText
                | LineColor
                | LineSewType
                | RegionColor
                | RegionSewType
                | EntryExit
                | SewingPoints
        )
    }

    /// On/off commands (drawn pressed while on).
    pub fn is_toggle(self) -> bool {
        use Action::*;
        matches!(
            self,
            SolidView
                | StitchView
                | RealisticView
                | ViewThreadTrimming
                | PaneSewingOrder
                | PaneSimulator
                | PaneAttributes
                | PaneImport
                | ReferenceWindow
                | ShowGrid
                | GridAxes
                | SnapToGrid
                | Ruler
                | Guideline
                | Pan
                | Measure
        )
    }

    /// The drawing tool the command picks, if it is one.
    pub fn tool(self) -> Option<Tool> {
        use Action::*;
        Some(match self {
            SelectTool => Tool::Select,
            PointTool | EditPoint => Tool::Point,
            Rectangle => Tool::Rectangle,
            Ellipse => Tool::Ellipse,
            ClosedStraight => Tool::Polygon,
            ClosedCurve | OpenCurve => Tool::Pen,
            StraightBlock => Tool::Punch,
            _ => return None,
        })
    }
}

/// Ribbon tabs: the three fixed ones and the contextual ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TabId {
    Home,
    Image,
    View,
    Text,
    Outline,
    OutlineRect,
    ManualPunch,
    Shapes,
    Stitches,
    HoopSettings,
}

impl TabId {
    pub const FIXED: [TabId; 3] = [TabId::Home, TabId::Image, TabId::View];
    pub const ALL: [TabId; 10] = [
        TabId::Home,
        TabId::Image,
        TabId::View,
        TabId::Text,
        TabId::Outline,
        TabId::OutlineRect,
        TabId::ManualPunch,
        TabId::Shapes,
        TabId::Stitches,
        TabId::HoopSettings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TabId::Home => "Home",
            TabId::Image => "Image",
            TabId::View => "View",
            TabId::Text => "Text",
            TabId::Outline | TabId::OutlineRect => "Outline",
            TabId::ManualPunch => "Manual Punch",
            TabId::Shapes => "Shapes",
            TabId::Stitches => "Stitches",
            TabId::HoopSettings => "Hoop Settings",
        }
    }

    /// Caption over a contextual tab (what it is for); `None` for the fixed tabs.
    pub fn context(self) -> Option<&'static str> {
        match self {
            TabId::Home | TabId::Image | TabId::View => None,
            TabId::Text => Some("Text"),
            TabId::Outline => Some("Shapes (Outline)"),
            TabId::OutlineRect => Some("Shapes (Rectangle)"),
            TabId::ManualPunch => Some("Shapes (Manual Punch)"),
            TabId::Shapes => Some("Shapes (Shapes/Circle/Arc)"),
            TabId::Stitches => Some("Stitches"),
            TabId::HoopSettings => Some("Hoop Settings"),
        }
    }

    /// `--ribbon` value.
    pub fn key(self) -> &'static str {
        match self {
            TabId::Home => "home",
            TabId::Image => "image",
            TabId::View => "view",
            TabId::Text => "text",
            TabId::Outline => "outline",
            TabId::OutlineRect => "rectangle",
            TabId::ManualPunch => "punch",
            TabId::Shapes => "shapes",
            TabId::Stitches => "stitches",
            TabId::HoopSettings => "hoop",
        }
    }

    pub fn from_key(key: &str) -> Option<TabId> {
        TabId::ALL.into_iter().find(|t| t.key() == key)
    }

    pub fn groups(self) -> &'static [Group] {
        match self {
            TabId::Home => HOME,
            TabId::Image => IMAGE,
            TabId::View => VIEW,
            TabId::Text => TEXT,
            TabId::Outline => OUTLINE,
            TabId::OutlineRect => OUTLINE_RECT,
            TabId::ManualPunch => MANUAL_PUNCH,
            TabId::Shapes => SHAPES,
            TabId::Stitches => STITCHES,
            TabId::HoopSettings => HOOP,
        }
    }
}

/// One column of a ribbon group: a large button, or up to three small ones stacked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Large(Action),
    Small(&'static [Action]),
}

/// A ribbon group: its buttons and the caption under them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Group {
    pub name: &'static str,
    pub items: &'static [Item],
}

impl Group {
    pub fn actions(&self) -> impl Iterator<Item = Action> + '_ {
        self.items
            .iter()
            .flat_map(|i| match i {
                Item::Large(a) => std::slice::from_ref(a),
                Item::Small(a) => a,
            })
            .copied()
    }
}

use Action as A;
use Item::{Large, Small};

const SELECT: Group = Group {
    name: "Select",
    items: &[Large(A::SelectTool), Small(&[A::PointTool, A::SelectAll])],
};

const COLORS: Group = Group {
    name: "Color / Sew Type",
    items: &[
        Small(&[A::LineColor, A::LineSewType]),
        Small(&[A::RegionColor, A::RegionSewType]),
    ],
};

const HOME: &[Group] = &[
    SELECT,
    Group {
        name: "Clipboard",
        items: &[
            Large(A::Paste),
            Small(&[A::Cut, A::Copy, A::Duplicate]),
            Small(&[A::Delete, A::ArrangeCopy]),
        ],
    },
    Group {
        name: "Tools",
        items: &[
            Large(A::TextTools),
            Large(A::ShapesTools),
            Small(&[A::ZoomIn, A::ZoomOut, A::OtherZoom]),
            Small(&[A::Pan, A::Measure]),
        ],
    },
    Group {
        name: "Import",
        items: &[Large(A::ImportPatterns), Small(&[A::ImportSvg])],
    },
    Group {
        name: "Edit",
        items: &[
            Small(&[A::Arrange, A::RotateFlip, A::Size]),
            Small(&[A::Group, A::Ungroup, A::FitTextToOutline]),
            Small(&[A::StampEmboss, A::ModifyOverlap, A::DecorativeFrame]),
        ],
    },
    Group {
        name: "Wizards",
        items: &[
            Large(A::BackgroundFill),
            Small(&[A::Applique, A::Patch, A::Cutwork]),
        ],
    },
    Group {
        name: "Sew",
        items: &[Large(A::SendEmbroidery)],
    },
];

const IMAGE: &[Group] = &[
    Group {
        name: "Image",
        items: &[
            Large(A::OpenImage),
            Small(&[A::PasteImage, A::SaveImage, A::ModifyImage]),
            Small(&[A::ImageDensity, A::ImageRotateRight, A::ImageRotateLeft]),
            Large(A::DeleteImage),
        ],
    },
    Group {
        name: "Image to Stitch",
        items: &[
            Large(A::AutoPunch),
            Large(A::CrossStitch),
            Large(A::PhotoStitch1),
            Large(A::PhotoStitch2),
        ],
    },
    Group {
        name: "Design Center",
        items: &[Large(A::DesignCenter)],
    },
];

const VIEW: &[Group] = &[
    Group {
        name: "View",
        items: &[
            Large(A::SolidView),
            Large(A::StitchView),
            Large(A::RealisticView),
            Small(&[A::ViewThreadTrimming, A::RealisticSettings]),
        ],
    },
    Group {
        name: "Show/Hide",
        items: &[
            Small(&[A::PaneSewingOrder, A::PaneSimulator, A::PaneAttributes]),
            Small(&[A::PaneImport, A::ReferenceWindow]),
        ],
    },
    Group {
        name: "Grid",
        items: &[
            Large(A::ShowGrid),
            Small(&[A::GridAxes, A::SnapToGrid, A::GridSpacing]),
        ],
    },
    Group {
        name: "Ruler",
        items: &[Large(A::Ruler)],
    },
    Group {
        name: "Guideline",
        items: &[Large(A::Guideline)],
    },
];

const TEXT: &[Group] = &[
    SELECT,
    Group {
        name: "Text",
        items: &[
            Large(A::TextTools),
            Small(&[A::Font, A::TextSize, A::FontFilter]),
            Small(&[A::FontFilterSettings, A::TrueTypeAttributes]),
        ],
    },
    COLORS,
    Group {
        name: "Edit",
        items: &[
            Large(A::EditText),
            Small(&[A::FitTextToOutline]),
            Small(&[
                A::ConvertToBlocks,
                A::ConvertToStitches,
                A::ConvertToOutline,
            ]),
        ],
    },
];

const OUTLINE: &[Group] = &[
    SELECT,
    Group {
        name: "Shapes",
        items: &[
            Small(&[A::ClosedStraight, A::ClosedCurve, A::PencilClosed]),
            Small(&[A::OpenStraight, A::OpenCurve, A::PencilOpen]),
        ],
    },
    COLORS,
];

const OUTLINE_RECT: &[Group] = &[
    SELECT,
    Group {
        name: "Shapes",
        items: &[Large(A::Rectangle), Small(&[A::EdgeRadius])],
    },
    COLORS,
];

const MANUAL_PUNCH: &[Group] = &[
    SELECT,
    Group {
        name: "Manual Punch",
        items: &[
            Small(&[A::StraightBlock, A::CurvedBlock]),
            Small(&[A::Running, A::Feed]),
        ],
    },
    Group {
        name: "Color / Sew Type",
        items: &[Small(&[A::RegionColor, A::RegionSewType])],
    },
];

const SHAPES: &[Group] = &[
    SELECT,
    Group {
        name: "Edit Outline",
        items: &[
            Large(A::SplitOutline),
            Small(&[
                A::AddDirectionLine,
                A::DeleteDirectionLine,
                A::ReverseDirectionLine,
            ]),
            Small(&[A::EditPoint, A::DeletePoint, A::SplitAtPoint]),
        ],
    },
    COLORS,
    Group {
        name: "Convert",
        items: &[Small(&[A::ConvertRegionToLine, A::ConvertToStitches])],
    },
    Group {
        name: "Create",
        items: &[
            Small(&[A::OffsetLines, A::FloralPattern]),
            Small(&[A::EntryExit, A::SewingPoints]),
        ],
    },
];

const STITCHES: &[Group] = &[
    SELECT,
    Group {
        name: "Edit",
        items: &[
            Large(A::SplitStitchData),
            Small(&[A::ChangeColor, A::DivideByColor, A::SplitAtPoint]),
            Small(&[A::Connect]),
        ],
    },
    Group {
        name: "Convert",
        items: &[Small(&[
            A::ConvertToBlocks,
            A::ConvertToOutline,
            A::ConvertToOutlines,
        ])],
    },
];

const HOOP: &[Group] = &[
    SELECT,
    Group {
        name: "Hoop Settings",
        items: &[
            Small(&[A::MoveSections]),
            Large(A::DesignSettings),
            Large(A::ExportSections),
        ],
    },
];

/// Application button menu, top to bottom; [`Action::Options`] and [`Action::Exit`] sit at
/// the bottom.
pub const APPLICATION_MENU: [Action; 11] = [
    A::New,
    A::Open,
    A::Save,
    A::SaveAs,
    A::Print,
    A::DesignProperty,
    A::DesignSettings,
    A::FabricSelector,
    A::SelectColorPalette,
    A::Wizard,
    A::Export,
];

/// The Option button: other applications and settings.
pub const OPTION_MENU: [Action; 6] = [
    A::DesignDatabase,
    A::StitchCreator,
    A::StitchDesignFactory,
    A::FontCreator,
    A::AttributeSets,
    A::UserThreadCharts,
];

/// Quick Access Toolbar default.
pub const QAT_DEFAULT: [Action; 7] = [
    A::New,
    A::Open,
    A::Save,
    A::Undo,
    A::Redo,
    A::OtherZoom,
    A::Help,
];

/// Commands that may be added to the Quick Access Toolbar.
pub fn qat_candidates() -> Vec<Action> {
    let mut v: Vec<Action> = QAT_DEFAULT.to_vec();
    for a in [
        A::SaveAs,
        A::Print,
        A::Export,
        A::ImportPatterns,
        A::DesignSettings,
        A::DesignProperty,
        A::ZoomIn,
        A::ZoomOut,
        A::Duplicate,
        A::Delete,
        A::SelectAll,
        A::Group,
        A::Ungroup,
        A::Measure,
        A::RealisticView,
        A::ShowGrid,
        A::SendEmbroidery,
        A::DesignDatabase,
    ] {
        if !v.contains(&a) {
            v.push(a);
        }
    }
    v
}

/// What decides the contextual tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Facts {
    pub tool: Tool,
    /// The selection holds text, shapes, stitch patterns.
    pub text: bool,
    pub shape: bool,
    pub stitches: bool,
    /// The design is larger than one hooping.
    pub sections: bool,
}

impl Default for Facts {
    fn default() -> Self {
        Facts {
            tool: Tool::Select,
            text: false,
            shape: false,
            stitches: false,
            sections: false,
        }
    }
}

/// Contextual tabs shown for the active tool and the selection, in ribbon order.
pub fn contextual_tabs(f: &Facts) -> Vec<TabId> {
    let mut v = Vec::new();
    let mut add = |t: TabId| {
        if !v.contains(&t) {
            v.push(t);
        }
    };
    match f.tool {
        Tool::Polygon | Tool::Pen => add(TabId::Outline),
        Tool::Rectangle => add(TabId::OutlineRect),
        Tool::Punch => add(TabId::ManualPunch),
        Tool::Ellipse | Tool::Point => add(TabId::Shapes),
        Tool::Select => {}
    }
    if f.text {
        add(TabId::Text);
    }
    if f.shape {
        add(TabId::Shapes);
    }
    if f.stitches {
        add(TabId::Stitches);
    }
    if f.sections {
        add(TabId::HoopSettings);
    }
    v
}

/// The contextual tab a tool brings forward when it is picked.
pub fn tab_for_tool(tool: Tool) -> Option<TabId> {
    contextual_tabs(&Facts {
        tool,
        ..Facts::default()
    })
    .first()
    .copied()
}

/// Every action placed on a ribbon tab.
pub fn ribbon_actions() -> Vec<Action> {
    let mut v = Vec::new();
    for t in TabId::ALL {
        for g in t.groups() {
            for a in g.actions() {
                if !v.contains(&a) {
                    v.push(a);
                }
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_round_trip() {
        let mut keys: Vec<_> = Action::ALL.iter().map(|a| a.info().key).collect();
        for &a in Action::ALL {
            assert_eq!(Action::from_key(a.info().key), Some(a));
        }
        keys.sort_unstable();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n, "duplicate action key");
        for t in TabId::ALL {
            assert_eq!(TabId::from_key(t.key()), Some(t));
        }
    }

    #[test]
    fn every_action_has_a_place_and_a_screen_tip() {
        let ribbon = ribbon_actions();
        // Copy/Paste Attributes live in the Sewing Attributes pane.
        let help = [
            A::Undo,
            A::Redo,
            A::Help,
            A::Options,
            A::Exit,
            A::CopyAttributes,
            A::PasteAttributes,
        ];
        for &a in Action::ALL {
            let placed = ribbon.contains(&a)
                || APPLICATION_MENU.contains(&a)
                || OPTION_MENU.contains(&a)
                || help.contains(&a)
                || a.tool().is_some()
                || [A::Ellipse, A::Rectangle].contains(&a);
            assert!(placed, "{a:?} is in no tab or menu");
            let info = a.info();
            assert!(!info.label.is_empty() && !info.tip.is_empty(), "{a:?}");
            let tip = info.screen_tip(false);
            assert_eq!(tip.contains("Not yet available"), !info.available, "{a:?}");
            if let Some(s) = info.shortcut {
                assert!(tip.contains(s), "{a:?}");
            }
        }
        assert_eq!(A::Save.info().shortcut_for(true).as_deref(), Some("Cmd+S"));
    }

    #[test]
    fn groups_are_named_and_columns_hold_at_most_three() {
        for t in TabId::ALL {
            assert!(!t.groups().is_empty(), "{t:?}");
            for g in t.groups() {
                assert!(!g.name.is_empty());
                for i in g.items {
                    if let Item::Small(a) = i {
                        assert!((1..=3).contains(&a.len()), "{t:?} {}", g.name);
                    }
                }
            }
            assert_eq!(t.context().is_none(), TabId::FIXED.contains(&t));
        }
        let home: Vec<&str> = TabId::Home.groups().iter().map(|g| g.name).collect();
        assert_eq!(
            home,
            [
                "Select",
                "Clipboard",
                "Tools",
                "Import",
                "Edit",
                "Wizards",
                "Sew"
            ]
        );
    }

    #[test]
    fn contextual_tabs_follow_tool_and_selection() {
        let f = |tool, text, shape, stitches, sections| Facts {
            tool,
            text,
            shape,
            stitches,
            sections,
        };
        assert!(contextual_tabs(&Facts::default()).is_empty());
        assert_eq!(
            contextual_tabs(&f(Tool::Polygon, false, false, false, false)),
            [TabId::Outline]
        );
        assert_eq!(
            contextual_tabs(&f(Tool::Pen, false, false, false, false)),
            [TabId::Outline]
        );
        assert_eq!(tab_for_tool(Tool::Rectangle), Some(TabId::OutlineRect));
        assert_eq!(tab_for_tool(Tool::Punch), Some(TabId::ManualPunch));
        assert_eq!(tab_for_tool(Tool::Ellipse), Some(TabId::Shapes));
        assert_eq!(tab_for_tool(Tool::Select), None);
        assert_eq!(
            contextual_tabs(&f(Tool::Select, true, true, true, true)),
            [
                TabId::Text,
                TabId::Shapes,
                TabId::Stitches,
                TabId::HoopSettings
            ]
        );
        assert_eq!(
            contextual_tabs(&f(Tool::Point, false, true, false, false)),
            [TabId::Shapes],
            "no duplicates"
        );
    }

    #[test]
    fn every_engine_command_named_exists() {
        let session = openstitch_engine::Session::new(std::env::temp_dir());
        let ids: Vec<&str> = session.commands().iter().map(|c| c.id).collect();
        for &a in Action::ALL {
            if let Some(e) = a.info().engine {
                assert!(ids.contains(&e), "{a:?} names unknown command {e}");
            }
        }
    }

    #[test]
    fn quick_access_toolbar_defaults() {
        let c = qat_candidates();
        assert!(QAT_DEFAULT.iter().all(|a| c.contains(a)));
        assert_eq!(
            QAT_DEFAULT.map(|a| a.info().label),
            ["New", "Open", "Save", "Undo", "Redo", "Zoom", "Help"]
        );
    }
}
