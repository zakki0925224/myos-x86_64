use crate::graphics::color::ColorCode;

pub const GLOBAL_THEME: Theme = LEGACY_THEME;

const LEGACY_BLACK: ColorCode = ColorCode::BLACK;
const LEGACY_DARK_GREEN: ColorCode = ColorCode::new_rgb(0x00, 0x55, 0x00);
const LEGACY_GREEN: ColorCode = ColorCode::new_rgb(0x00, 0xaa, 0x00);
const LEGACY_BRIGHT_GREEN: ColorCode = ColorCode::new_rgb(0x00, 0xff, 0x00);
const LEGACY_BLUE: ColorCode = ColorCode::new_rgb(0x00, 0x00, 0xff);
const LEGACY_MODERATE_BLUE: ColorCode = ColorCode::new_rgb(0x00, 0x55, 0xaa);
const LEGACY_LIGHT_BLUE: ColorCode = ColorCode::new_rgb(0x00, 0xaa, 0xff);
const LEGACY_CYAN: ColorCode = ColorCode::new_rgb(0x00, 0xff, 0xff);
const LEGACY_RED: ColorCode = ColorCode::RED;
const LEGACY_ORANGE: ColorCode = ColorCode::new_rgb(0xff, 0x55, 0x00);
const LEGACY_YELLOW_ORANGE: ColorCode = ColorCode::new_rgb(0xff, 0xaa, 0x00);
const LEGACY_YELLOW: ColorCode = ColorCode::new_rgb(0xff, 0xff, 0x00);
const LEGACY_MAGENTA: ColorCode = ColorCode::new_rgb(0xff, 0x00, 0xff);
const LEGACY_BRIGHT_MAGENTA: ColorCode = ColorCode::new_rgb(0xff, 0x55, 0xff);
const LEGACY_SOFT_MAGENTA: ColorCode = ColorCode::new_rgb(0xff, 0xaa, 0xff);
const LEGACY_WHITE: ColorCode = ColorCode::WHITE;

const EGA_BLACK: ColorCode = ColorCode::BLACK;
const EGA_BLUE: ColorCode = ColorCode::new_rgb(0x00, 0x00, 0xaa);
const EGA_GREEN: ColorCode = ColorCode::new_rgb(0x00, 0xaa, 0x00);
const EGA_CYAN: ColorCode = ColorCode::new_rgb(0x00, 0xaa, 0xaa);
const EGA_RED: ColorCode = ColorCode::new_rgb(0xaa, 0x00, 0x00);
const EGA_MAGENTA: ColorCode = ColorCode::new_rgb(0xaa, 0x00, 0xaa);
const EGA_BROWN: ColorCode = ColorCode::new_rgb(0xaa, 0x55, 0x00);
const EGA_LIGHT_GRAY: ColorCode = ColorCode::new_rgb(0xaa, 0xaa, 0xaa);
const EGA_DARK_GRAY: ColorCode = ColorCode::new_rgb(0x55, 0x55, 0x55);
const EGA_LIGHT_BLUE: ColorCode = ColorCode::new_rgb(0x55, 0x55, 0xff);
const EGA_LIGHT_GREEN: ColorCode = ColorCode::new_rgb(0x55, 0xff, 0x55);
const EGA_LIGHT_CYAN: ColorCode = ColorCode::new_rgb(0x55, 0xff, 0xff);
const EGA_LIGHT_RED: ColorCode = ColorCode::new_rgb(0xff, 0x55, 0x55);
const EGA_LIGHT_MAGENTA: ColorCode = ColorCode::new_rgb(0xff, 0x55, 0xff);
const EGA_YELLOW: ColorCode = ColorCode::new_rgb(0xff, 0xff, 0x55);
const EGA_WHITE: ColorCode = ColorCode::WHITE;

#[allow(unused)]
const LEGACY_THEME: Theme = Theme {
    console: ConsoleTheme {
        back: ColorCode::new_rgb(0x03, 0x1a, 0x00),
        fore: LEGACY_GREEN,
        palette: [
            LEGACY_BLACK,
            LEGACY_DARK_GREEN,
            LEGACY_GREEN,
            LEGACY_BRIGHT_GREEN,
            LEGACY_BLUE,
            LEGACY_MODERATE_BLUE,
            LEGACY_LIGHT_BLUE,
            LEGACY_CYAN,
            LEGACY_RED,
            LEGACY_ORANGE,
            LEGACY_YELLOW_ORANGE,
            LEGACY_YELLOW,
            LEGACY_MAGENTA,
            LEGACY_BRIGHT_MAGENTA,
            LEGACY_SOFT_MAGENTA,
            LEGACY_WHITE,
        ],
    },
    log: LogTheme {
        error: LEGACY_RED,
        warn: LEGACY_ORANGE,
        info: LEGACY_CYAN,
        debug: LEGACY_YELLOW,
        trace: LEGACY_MAGENTA,
    },
    wm: WmTheme {
        component_back: LEGACY_BLACK,
        component_fore: LEGACY_GREEN,
        border_color1: LEGACY_GREEN,
        border_color2: LEGACY_GREEN,
        border_flat: true,
        titlebar_back: LEGACY_BLACK,
        titlebar_fore: LEGACY_GREEN,
    },
};

#[allow(unused)]
const CLASSIC_BACK: ColorCode = ColorCode::new_rgb(0x3a, 0x6e, 0xa5);
const CLASSIC_FORE: ColorCode = ColorCode::new_rgb(0xd4, 0xd0, 0xc8);

#[allow(unused)]
const CLASSIC_THEME: Theme = Theme {
    console: ConsoleTheme {
        back: CLASSIC_BACK,
        fore: ColorCode::WHITE,
        palette: [
            CLASSIC_BACK,
            CLASSIC_BACK,
            CLASSIC_BACK,
            CLASSIC_BACK,
            CLASSIC_BACK,
            CLASSIC_BACK,
            CLASSIC_BACK,
            CLASSIC_BACK,
            CLASSIC_BACK,
            CLASSIC_BACK,
            CLASSIC_BACK,
            CLASSIC_BACK,
            CLASSIC_BACK,
            CLASSIC_BACK,
            CLASSIC_BACK,
            CLASSIC_BACK,
        ],
    },
    log: LogTheme {
        error: ColorCode::RED,
        warn: ColorCode::new_rgb(0xe7, 0xe7, 0x00),
        info: ColorCode::new_rgb(0x00, 0xc0, 0x00),
        debug: ColorCode::WHITE,
        trace: ColorCode::WHITE,
    },
    wm: WmTheme {
        component_back: CLASSIC_FORE,
        component_fore: ColorCode::BLACK,
        border_color1: ColorCode::WHITE,
        border_color2: ColorCode::new_rgb(0x79, 0x75, 0x71),
        border_flat: false,
        titlebar_back: ColorCode::new_rgb(0x0a, 0x24, 0x6a),
        titlebar_fore: ColorCode::WHITE,
    },
};

#[allow(unused)]
const EGA_THEME: Theme = Theme {
    console: ConsoleTheme {
        back: EGA_BLACK,
        fore: EGA_CYAN,
        palette: [
            EGA_BLACK,
            EGA_BLUE,
            EGA_GREEN,
            EGA_CYAN,
            EGA_RED,
            EGA_MAGENTA,
            EGA_BROWN,
            EGA_LIGHT_GRAY,
            EGA_DARK_GRAY,
            EGA_LIGHT_BLUE,
            EGA_LIGHT_GREEN,
            EGA_LIGHT_CYAN,
            EGA_LIGHT_RED,
            EGA_LIGHT_MAGENTA,
            EGA_YELLOW,
            EGA_WHITE,
        ],
    },
    log: LogTheme {
        error: EGA_LIGHT_RED,
        warn: EGA_YELLOW,
        info: EGA_LIGHT_GREEN,
        debug: EGA_WHITE,
        trace: EGA_LIGHT_MAGENTA,
    },
    wm: WmTheme {
        component_back: EGA_BLACK,
        component_fore: EGA_LIGHT_CYAN,
        border_color1: EGA_LIGHT_CYAN,
        border_color2: EGA_LIGHT_CYAN,
        border_flat: true,
        titlebar_back: EGA_BLACK,
        titlebar_fore: EGA_LIGHT_CYAN,
    },
};

#[allow(unused)]
pub struct Theme {
    pub console: ConsoleTheme,
    pub log: LogTheme,
    pub wm: WmTheme,
}

pub struct ConsoleTheme {
    pub back: ColorCode,
    pub fore: ColorCode,
    pub palette: [ColorCode; 16],
}

pub struct LogTheme {
    pub error: ColorCode,
    pub warn: ColorCode,
    pub info: ColorCode,
    pub debug: ColorCode,
    pub trace: ColorCode,
}

pub struct WmTheme {
    pub component_back: ColorCode,
    pub component_fore: ColorCode,
    pub border_color1: ColorCode, // left, top
    pub border_color2: ColorCode, // right, bottom
    pub border_flat: bool,
    pub titlebar_back: ColorCode,
    pub titlebar_fore: ColorCode,
}
