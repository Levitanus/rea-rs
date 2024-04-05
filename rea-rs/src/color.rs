use crate::Reaper;
use serde_derive::{Deserialize, Serialize};
use std::mem::MaybeUninit;

#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
impl Color {
    /// New color from r, g, b (0..255).
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Get as tuple.
    pub fn get(&self) -> (u8, u8, u8) {
        (self.r, self.g, self.b)
    }

    /// Make from the OS-dependent color.
    pub fn from_native(native: i32) -> Self {
        unsafe {
            let low = Reaper::get().low();
            let (mut r, mut g, mut b) = (
                MaybeUninit::new(0),
                MaybeUninit::new(0),
                MaybeUninit::new(0),
            );
            low.ColorFromNative(
                native,
                r.as_mut_ptr(),
                g.as_mut_ptr(),
                b.as_mut_ptr(),
            );
            Self {
                r: r.assume_init_read() as u8,
                g: g.assume_init_read() as u8,
                b: b.assume_init_read() as u8,
            }
        }
    }

    /// Convert to OS-dependent color.
    pub fn to_native(&self) -> i32 {
        let low = Reaper::get().low();
        low.ColorToNative(self.r as i32, self.g as i32, self.b as i32)
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, strum::Display)]
pub enum ThemeColor {
    /// Main window/transport background
    /// current RGB: 51,51,51
    col_main_bg2,
    /// Main window/transport text
    /// current RGB: 170,170,170
    col_main_text2,
    /// Main window text shadow (ignored if too close to text color)
    /// current RGB: 18,26,29
    col_main_textshadow,
    /// Main window 3D highlight
    /// current RGB: 70,70,70
    col_main_3dhl,
    /// Main window 3D shadow
    /// current RGB: 45,45,45
    col_main_3dsh,
    /// Main window pane resize mouseover
    /// current RGB: 51,51,51
    col_main_resize2,
    /// Themed window text
    /// current RGB: 18,18,18
    col_main_text,
    /// Themed window background
    /// current RGB: 123,123,123
    col_main_bg,
    /// Themed window edit background
    /// current RGB: 170,170,170
    col_main_editbk,
    /// Do not use window theming on macOS dark mode
    /// bool 00000000
    col_nodarkmodemiscwnd,
    /// Transport edit background
    /// current RGB: 51,51,51
    col_transport_editbk,
    /// Toolbar button text
    /// current RGB: 159,159,159
    col_toolbar_text,
    /// Toolbar button enabled text
    /// current RGB: 191,251,192
    col_toolbar_text_on,
    /// Toolbar frame when floating or docked
    /// current RGB: 71,78,78
    col_toolbar_frame,
    /// Toolbar button armed color
    /// current RGB: 255,128,0
    toolbararmed_color,
    /// Toolbar button armed fill mode
    /// blendmode 00028001
    toolbararmed_drawmode,
    /// I/O window text
    /// current RGB: 69,69,69
    io_text,
    /// I/O window 3D highlight
    /// current RGB: 131,131,131
    io_3dhl,
    /// I/O window 3D shadow
    /// current RGB: 204,204,204
    io_3dsh,
    /// Window list background
    /// current RGB: 170,170,170
    genlist_bg,
    /// Window list text
    /// current RGB: 18,18,18
    genlist_fg,
    /// Window list grid lines
    /// current RGB: 132,132,132
    genlist_grid,
    /// Window list selected row
    /// current RGB: 87,87,87
    genlist_selbg,
    /// Window list selected text
    /// current RGB: 255,255,255
    genlist_selfg,
    /// Window list selected row (inactive)
    /// current RGB: 240,240,240
    genlist_seliabg,
    /// Window list selected text (inactive)
    /// current RGB: 0,0,0
    genlist_seliafg,
    /// Window list highlighted text
    /// current RGB: 0,0,224
    genlist_hilite,
    /// Window list highlighted selected text
    /// current RGB: 192,192,255
    genlist_hilite_sel,
    /// Button background
    /// current RGB: 0,0,0
    col_buttonbg,
    /// Track panel text
    /// current RGB: 18,26,29
    col_tcp_text,
    /// Track panel (selected) text
    /// current RGB: 18,26,29
    col_tcp_textsel,
    /// Selected track control panel background
    /// current RGB: 210,210,210
    col_seltrack,
    /// Unselected track control panel background (enabled with a checkbox
    /// above) current RGB: 197,197,197
    col_seltrack2,
    /// Locked track control panel overlay color
    /// current RGB: 51,51,51
    tcplocked_color,
    /// Locked track control panel fill mode
    /// blendmode 0002c000
    tcplocked_drawmode,
    /// Empty track list area
    /// current RGB: 51,51,51
    col_tracklistbg,
    /// Empty mixer list area
    /// current RGB: 51,51,51
    col_mixerbg,
    /// Empty arrange view area
    /// current RGB: 41,41,41
    col_arrangebg,
    /// Empty arrange view area vertical grid shading
    /// current RGB: 41,41,41
    arrange_vgrid,
    /// Fader background when automation recording
    /// current RGB: 255,125,125
    col_fadearm,
    /// Fader background when automation playing
    /// current RGB: 125,255,125
    col_fadearm2,
    /// Fader background when in inactive touch/latch
    /// current RGB: 255,255,98
    col_fadearm3,
    /// Timeline foreground
    /// current RGB: 105,107,107
    col_tl_fg,
    /// Timeline foreground (secondary markings)
    /// current RGB: 76,77,77
    col_tl_fg2,
    /// Timeline background
    /// current RGB: 48,48,48
    col_tl_bg,
    /// Time selection color
    /// current RGB: 255,255,255
    col_tl_bgsel,
    /// Time selection fill mode
    /// blendmode 00020f01
    timesel_drawmode,
    /// Timeline background (in loop points)
    /// current RGB: 192,192,192
    col_tl_bgsel2,
    /// Transport status background
    /// current RGB: 73,73,73
    col_trans_bg,
    /// Transport status text
    /// current RGB: 137,139,139
    col_trans_fg,
    /// Project play rate control when not 1.0
    /// current RGB: 127,63,0
    playrate_edited,
    /// Media item selection indicator
    /// current RGB: 255,255,255
    selitem_dot,
    /// Media item label
    /// current RGB: 170,170,170
    col_mi_label,
    /// Media item label (selected)
    /// current RGB: 170,170,170
    col_mi_label_sel,
    /// Floating media item label
    /// current RGB: 170,170,170
    col_mi_label_float,
    /// Floating media item label (selected)
    /// current RGB: 170,170,170
    col_mi_label_float_sel,
    /// Media item background (odd tracks)
    /// current RGB: 125,125,125
    col_mi_bg,
    /// Media item background (even tracks)
    /// current RGB: 125,125,125
    col_mi_bg2,
    /// Media item background selected (odd tracks)
    /// current RGB: 106,255,163
    col_tr1_itembgsel,
    /// Media item background selected (even tracks)
    /// current RGB: 106,255,163
    col_tr2_itembgsel,
    /// Media item background fill mode
    /// blendmode 00028000
    itembg_drawmode,
    /// Media item peaks (odd tracks)
    /// current RGB: 21,21,21
    col_tr1_peaks,
    /// Media item peaks (even tracks)
    /// current RGB: 21,21,21
    col_tr2_peaks,
    /// Media item peaks when selected (odd tracks)
    /// current RGB: 21,21,21
    col_tr1_ps2,
    /// Media item peaks when selected (even tracks)
    /// current RGB: 21,21,21
    col_tr2_ps2,
    /// Media item peaks edge highlight (odd tracks)
    /// current RGB: 51,51,51
    col_peaksedge,
    /// Media item peaks edge highlight (even tracks)
    /// current RGB: 51,51,51
    col_peaksedge2,
    /// Media item peaks edge highlight when selected (odd tracks)
    /// current RGB: 51,51,51
    col_peaksedgesel,
    /// Media item peaks edge highlight when selected (even tracks)
    /// current RGB: 51,51,51
    col_peaksedgesel2,
    /// Media item MIDI CC peaks fill mode
    /// blendmode 00024000
    cc_chase_drawmode,
    /// Media item peaks when active in crossfade editor (fade-out)
    /// current RGB: 0,255,0
    col_peaksfade,
    /// Media item peaks when active in crossfade editor (fade-in)
    /// current RGB: 255,0,0
    col_peaksfade2,
    /// Media item fade/volume controls
    /// current RGB: 105,16,16
    col_mi_fades,
    /// Media item fade quiet zone fill color
    /// current RGB: 72,0,0
    fadezone_color,
    /// Media item fade quiet zone fill mode
    /// blendmode 00030004
    fadezone_drawmode,
    /// Media item fade full area fill color
    /// current RGB: 0,0,96
    fadearea_color,
    /// Media item fade full area fill mode
    /// blendmode 00020000
    fadearea_drawmode,
    /// Media item edges of controls
    /// current RGB: 198,198,198
    col_mi_fade2,
    /// Media item edges of controls blend mode
    /// blendmode 00025901
    col_mi_fade2_drawmode,
    /// Media item edge when selected via grouping
    /// current RGB: 51,184,48
    item_grouphl,
    /// Media item "offline" text
    /// current RGB: 48,66,71
    col_offlinetext,
    /// Media item stretch marker line
    /// current RGB: 84,124,124
    col_stretchmarker,
    /// Media item stretch marker handle (1x)
    /// current RGB: 120,135,135
    col_stretchmarker_h0,
    /// Media item stretch marker handle (>1x)
    /// current RGB: 40,141,196
    col_stretchmarker_h1,
    /// Media item stretch marker handle (<1x)
    /// current RGB: 159,64,64
    col_stretchmarker_h2,
    /// Media item stretch marker handle edge
    /// current RGB: 192,192,192
    col_stretchmarker_b,
    /// Media item stretch marker blend mode
    /// blendmode 00030000
    col_stretchmarkerm,
    /// Media item stretch marker text
    /// current RGB: 126,153,154
    col_stretchmarker_text,
    /// Media item transient guide handle
    /// current RGB: 0,234,0
    col_stretchmarker_tm,
    /// Media item take marker
    /// current RGB: 255,255,0
    take_marker,
    /// Selected media item bar color
    /// current RGB: 0,0,0
    selitem_tag,
    /// Active media item take bar color
    /// current RGB: 0,0,0
    activetake_tag,
    /// Track background (odd tracks)
    /// current RGB: 41,41,41
    col_tr1_bg,
    /// Track background (even tracks)
    /// current RGB: 41,41,41
    col_tr2_bg,
    /// Selected track background (odd tracks)
    /// current RGB: 41,41,41
    selcol_tr1_bg,
    /// Selected track background (even tracks)
    /// current RGB: 41,41,41
    selcol_tr2_bg,
    /// Track fixed lane button
    /// current RGB: 87,96,87
    track_lane_tabcol,
    /// Track fixed lane button when only this lane plays
    /// current RGB: 201,201,38
    track_lanesolo_tabcol,
    /// Track fixed lane button text
    /// current RGB: 200,200,200
    track_lanesolo_text,
    /// Track fixed lane add area
    /// current RGB: 128,128,128
    track_lane_gutter,
    /// Track fixed lane add fill mode
    /// blendmode 00024000
    track_lane_gutter_drawmode,
    /// Track divider line (odd tracks)
    /// current RGB: 67,67,67
    col_tr1_divline,
    /// Track divider line (even tracks)
    /// current RGB: 67,67,67
    col_tr2_divline,
    /// Envelope lane divider line (odd tracks)
    /// current RGB: 0,0,0
    col_envlane1_divline,
    /// Envelope lane divider line (even tracks)
    /// current RGB: 0,0,0
    col_envlane2_divline,
    /// Muted/unsoloed track/item overlay color
    /// current RGB: 65,65,65
    mute_overlay_col,
    /// Muted/unsoloed track/item overlay mode
    /// blendmode 0002a600
    mute_overlay_mode,
    /// Inactive take/lane overlay color
    /// current RGB: 48,48,48
    inactive_take_overlay_col,
    /// Inactive take/lane overlay mode
    /// blendmode 00028000
    inactive_take_overlay_mode,
    /// Locked track/item overlay color
    /// current RGB: 0,0,0
    locked_overlay_col,
    /// Locked track/item overlay mode
    /// blendmode 00025c03
    locked_overlay_mode,
    /// Marquee fill
    /// current RGB: 128,128,110
    marquee_fill,
    /// Marquee fill mode
    /// blendmode 000299ff
    marquee_drawmode,
    /// Marquee outline
    /// current RGB: 255,255,255
    marquee_outline,
    /// Marquee zoom fill
    /// current RGB: 255,255,255
    marqueezoom_fill,
    /// Marquee zoom fill mode
    /// blendmode 00024002
    marqueezoom_drawmode,
    /// Marquee zoom outline
    /// current RGB: 0,255,0
    marqueezoom_outline,
    /// Razor edit area fill
    /// current RGB: 31,233,192
    areasel_fill,
    /// Razor edit area fill mode
    /// blendmode 00021c01
    areasel_drawmode,
    /// Razor edit area outline
    /// current RGB: 0,251,201
    areasel_outline,
    /// Razor edit area outline mode
    /// blendmode 0002c000
    areasel_outlinemode,
    /// Fixed lane comp area fill
    /// current RGB: 255,203,0
    linkedlane_fill,
    /// Fixed lane comp area fill mode
    /// blendmode 00020c01
    linkedlane_fillmode,
    /// Fixed lane comp area outline
    /// current RGB: 255,237,164
    linkedlane_outline,
    /// Fixed lane comp area outline mode
    /// blendmode 0002c000
    linkedlane_outlinemode,
    /// Fixed lane comp lane unsynced media item
    /// current RGB: 0,198,255
    linkedlane_unsynced,
    /// Fixed lane comp lane unsynced media item mode
    /// blendmode 0002ff00
    linkedlane_unsynced_mode,
    /// Edit cursor
    /// current RGB: 220,36,36
    col_cursor,
    /// Edit cursor (alternate)
    /// current RGB: 220,36,36
    col_cursor2,
    /// Play cursor
    /// current RGB: 0,0,0
    playcursor_color,
    /// Play cursor fill mode
    /// blendmode 00028003
    playcursor_drawmode,
    /// Grid lines (start of measure)
    /// current RGB: 138,69,0
    col_gridlines2,
    /// Grid lines (start of measure) - draw mode
    /// blendmode 0002c001
    col_gridlines2dm,
    /// Grid lines (start of beats)
    /// current RGB: 73,73,73
    col_gridlines3,
    /// Grid lines (start of beats) - draw mode
    /// blendmode 00028001
    col_gridlines3dm,
    /// Grid lines (in between beats)
    /// current RGB: 50,50,50
    col_gridlines,
    /// Grid lines (in between beats) - draw mode
    /// blendmode 00028001
    col_gridlines1dm,
    /// Editing guide line color
    /// current RGB: 95,169,167
    guideline_color,
    /// Editing guide fill mode
    /// blendmode 00024c01
    guideline_drawmode,
    /// Regions
    /// current RGB: 128,138,138
    region,
    /// Region lane background
    /// current RGB: 51,51,51
    region_lane_bg,
    /// Region lane text
    /// current RGB: 31,39,37
    region_lane_text,
    /// Markers
    /// current RGB: 75,0,0
    marker,
    /// Marker lane background
    /// current RGB: 73,73,73
    marker_lane_bg,
    /// Marker lane text
    /// current RGB: 165,165,165
    marker_lane_text,
    /// Time signature change marker
    /// current RGB: 170,170,170
    col_tsigmark,
    /// Time signature lane background
    /// current RGB: 51,51,51
    ts_lane_bg,
    /// Time signature lane text
    /// current RGB: 165,165,165
    ts_lane_text,
    /// Time signature marker selected background
    /// current RGB: 70,0,0
    timesig_sel_bg,
    /// Routing matrix row highlight
    /// current RGB: 255,255,192
    col_routinghl1,
    /// Routing matrix column highlight
    /// current RGB: 128,128,255
    col_routinghl2,
    /// Routing matrix input activity highlight
    /// current RGB: 64,255,64
    col_routingact,
    /// Theme has interlaced VU meters
    /// bool 00000000
    col_vudoint,
    /// VU meter clip indicator
    /// current RGB: 255,0,0
    col_vuclip,
    /// VU meter top
    /// current RGB: 255,128,0
    col_vutop,
    /// VU meter middle
    /// current RGB: 255,255,0
    col_vumid,
    /// VU meter bottom
    /// current RGB: 0,191,191
    col_vubot,
    /// VU meter interlace/edge color
    /// current RGB: 32,32,32
    col_vuintcol,
    /// VU meter gain reduction background
    /// current RGB: 32,32,32
    vu_gr_bgcol,
    /// VU meter gain reduction indicator
    /// current RGB: 224,224,0
    vu_gr_fgcol,
    /// VU meter midi activity
    /// current RGB: 255,0,0
    col_vumidi,
    /// VU (indicator) - no signal
    /// current RGB: 32,32,32
    col_vuind1,
    /// VU (indicator) - low signal
    /// current RGB: 213,0,0
    col_vuind2,
    /// VU (indicator) - med signal
    /// current RGB: 255,128,0
    col_vuind3,
    /// VU (indicator) - hot signal
    /// current RGB: 255,255,0
    col_vuind4,
    /// Sends text: normal
    /// current RGB: 163,163,163
    mcp_sends_normal,
    /// Sends text: muted
    /// current RGB: 152,134,99
    mcp_sends_muted,
    /// Sends text: MIDI hardware
    /// current RGB: 163,163,163
    mcp_send_midihw,
    /// Sends level
    /// current RGB: 48,66,71
    mcp_sends_levels,
    /// FX insert text: normal
    /// current RGB: 201,164,107
    mcp_fx_normal,
    /// FX insert text: bypassed
    /// current RGB: 140,140,140
    mcp_fx_bypassed,
    /// FX insert text: offline
    /// current RGB: 183,68,68
    mcp_fx_offlined,
    /// FX parameter text: normal
    /// current RGB: 163,163,163
    mcp_fxparm_normal,
    /// FX parameter text: bypassed
    /// current RGB: 152,134,99
    mcp_fxparm_bypassed,
    /// FX parameter text: offline
    /// current RGB: 152,99,99
    mcp_fxparm_offlined,
    /// List scrollbar (track panel)
    /// current RGB: 50,50,50
    tcp_list_scrollbar,
    /// List scrollbar (track panel) - draw mode
    /// blendmode 00028000
    tcp_list_scrollbar_mode,
    /// List scrollbar mouseover (track panel)
    /// current RGB: 30,30,30
    tcp_list_scrollbar_mouseover,
    /// List scrollbar mouseover (track panel) - draw mode
    /// blendmode 00028000
    tcp_list_scrollbar_mouseover_mode,
    /// List scrollbar (mixer panel)
    /// current RGB: 140,140,140
    mcp_list_scrollbar,
    /// List scrollbar (mixer panel) - draw mode
    /// blendmode 00028000
    mcp_list_scrollbar_mode,
    /// List scrollbar mouseover (mixer panel)
    /// current RGB: 64,191,159
    mcp_list_scrollbar_mouseover,
    /// List scrollbar mouseover (mixer panel) - draw mode
    /// blendmode 00028000
    mcp_list_scrollbar_mouseover_mode,
    /// MIDI editor ruler background
    /// current RGB: 51,51,51
    midi_rulerbg,
    /// MIDI editor ruler text
    /// current RGB: 127,127,127
    midi_rulerfg,
    /// MIDI editor grid line (start of measure)
    /// current RGB: 138,69,0
    midi_grid2,
    /// MIDI editor grid line (start of measure) - draw mode
    /// blendmode 00030000
    midi_griddm2,
    /// MIDI editor grid line (start of beats)
    /// current RGB: 91,91,91
    midi_grid3,
    /// MIDI editor grid line (start of beats) - draw mode
    /// blendmode 00030000
    midi_griddm3,
    /// MIDI editor grid line (between beats)
    /// current RGB: 91,91,91
    midi_grid1,
    /// MIDI editor grid line (between beats) - draw mode
    /// blendmode 00028000
    midi_griddm1,
    /// MIDI editor background color (naturals)
    /// current RGB: 48,48,48
    midi_trackbg1,
    /// MIDI editor background color (sharps/flats)
    /// current RGB: 41,41,41
    midi_trackbg2,
    /// MIDI editor background color, out of bounds (naturals)
    /// current RGB: 31,31,31
    midi_trackbg_outer1,
    /// MIDI editor background color, out of bounds (sharps/flats)
    /// current RGB: 24,24,24
    midi_trackbg_outer2,
    /// MIDI editor background color, selected pitch (naturals)
    /// current RGB: 87,57,57
    midi_selpitch1,
    /// MIDI editor background color, selected pitch (sharps/flats)
    /// current RGB: 75,52,52
    midi_selpitch2,
    /// MIDI editor time selection color
    /// current RGB: 255,255,255
    midi_selbg,
    /// MIDI editor time selection fill mode
    /// blendmode 00020c01
    midi_selbg_drawmode,
    /// MIDI editor CC horizontal center line
    /// current RGB: 157,157,157
    midi_gridhc,
    /// MIDI editor CC horizontal center line - draw mode
    /// blendmode 00030000
    midi_gridhcdm,
    /// MIDI editor CC horizontal line
    /// current RGB: 91,91,91
    midi_gridh,
    /// MIDI editor CC horizontal line - draw mode
    /// blendmode 00028000
    midi_gridhdm,
    /// MIDI editor CC lane add/remove buttons
    /// current RGB: 123,123,123
    midi_ccbut,
    /// MIDI editor CC lane button text
    /// current RGB: 170,170,170
    midi_ccbut_text,
    /// MIDI editor CC lane button arrow
    /// current RGB: 170,170,170
    midi_ccbut_arrow,
    /// MIDI editor octave line color
    /// current RGB: 73,73,73
    midioct,
    /// MIDI inline background color (naturals)
    /// current RGB: 48,48,48
    midi_inline_trackbg1,
    /// MIDI inline background color (sharps/flats)
    /// current RGB: 41,41,41
    midi_inline_trackbg2,
    /// MIDI inline octave line color
    /// current RGB: 73,73,73
    midioct_inline,
    /// MIDI editor end marker
    /// current RGB: 58,58,58
    midi_endpt,
    /// MIDI editor note, unselected (midi_note_colormap overrides)
    /// current RGB: 91,123,108
    midi_notebg,
    /// MIDI editor note, selected (midi_note_colormap overrides)
    /// current RGB: 49,49,49
    midi_notefg,
    /// MIDI editor note, muted, unselected (midi_note_colormap overrides)
    /// current RGB: 53,53,53
    midi_notemute,
    /// MIDI editor note, muted, selected (midi_note_colormap overrides)
    /// current RGB: 24,24,24
    midi_notemute_sel,
    /// MIDI editor note controls
    /// current RGB: 53,53,53
    midi_itemctl,
    /// MIDI editor note (offscreen)
    /// current RGB: 59,59,59
    midi_ofsn,
    /// MIDI editor note (offscreen, selected)
    /// current RGB: 59,59,59
    midi_ofsnsel,
    /// MIDI editor cursor
    /// current RGB: 220,36,36
    midi_editcurs,
    /// MIDI piano key color (naturals background, sharps/flats text)
    /// current RGB: 255,255,255
    midi_pkey1,
    /// MIDI piano key color (sharps/flats background, naturals text)
    /// current RGB: 0,0,0
    midi_pkey2,
    /// MIDI piano key color (selected)
    /// current RGB: 93,93,93
    midi_pkey3,
    /// MIDI piano key note-on flash
    /// current RGB: 188,148,39
    midi_noteon_flash,
    /// MIDI piano pane background
    /// current RGB: 53,53,53
    midi_leftbg,
    /// MIDI editor note text and control color, unselected (light)
    /// current RGB: 224,224,224
    midifont_col_light_unsel,
    /// MIDI editor note text and control color, unselected (dark)
    /// current RGB: 32,32,32
    midifont_col_dark_unsel,
    /// MIDI editor note text and control mode, unselected
    /// blendmode 0002c000
    midifont_mode_unsel,
    /// MIDI editor note text and control color (light)
    /// current RGB: 189,189,189
    midifont_col_light,
    /// MIDI editor note text and control color (dark)
    /// current RGB: 64,64,64
    midifont_col_dark,
    /// MIDI editor note text and control mode
    /// blendmode 00030000
    midifont_mode,
    /// MIDI notation editor background
    /// current RGB: 255,255,255
    score_bg,
    /// MIDI notation editor staff/notation/text
    /// current RGB: 0,0,0
    score_fg,
    /// MIDI notation editor selected staff/notation/text
    /// current RGB: 0,0,255
    score_sel,
    /// MIDI notation editor time selection
    /// current RGB: 255,255,224
    score_timesel,
    /// MIDI notation editor loop points, selected pitch
    /// current RGB: 255,192,0
    score_loop,
    /// MIDI list editor background
    /// current RGB: 53,53,53
    midieditorlist_bg,
    /// MIDI list editor text
    /// current RGB: 170,170,170
    midieditorlist_fg,
    /// MIDI list editor grid lines
    /// current RGB: 53,53,53
    midieditorlist_grid,
    /// MIDI list editor selected row
    /// current RGB: 51,153,255
    midieditorlist_selbg,
    /// MIDI list editor selected text
    /// current RGB: 255,255,255
    midieditorlist_selfg,
    /// MIDI list editor selected row (inactive)
    /// current RGB: 240,240,240
    midieditorlist_seliabg,
    /// MIDI list editor selected text (inactive)
    /// current RGB: 0,0,0
    midieditorlist_seliafg,
    /// MIDI list editor background (secondary)
    /// current RGB: 53,53,53
    midieditorlist_bg2,
    /// MIDI list editor text (secondary)
    /// current RGB: 0,0,0
    midieditorlist_fg2,
    /// MIDI list editor selected row (secondary)
    /// current RGB: 35,135,240
    midieditorlist_selbg2,
    /// MIDI list editor selected text (secondary)
    /// current RGB: 255,255,255
    midieditorlist_selfg2,
    /// Media explorer selection
    /// current RGB: 255,255,255
    col_explorer_sel,
    /// Media explorer selection mode
    /// blendmode 00021501
    col_explorer_seldm,
    /// Media explorer selection edge
    /// current RGB: 255,255,255
    col_explorer_seledge,
    /// Media explorer grid, markers
    /// current RGB: 255,255,255
    explorer_grid,
    /// Media explorer pitch detection text
    /// current RGB: 255,255,255
    explorer_pitchtext,
    /// Tab control shadow
    /// current RGB: 18,26,29
    docker_shadow,
    /// Tab control selected tab
    /// current RGB: 74,74,74
    docker_selface,
    /// Tab control unselected tab
    /// current RGB: 51,51,51
    docker_unselface,
    /// Tab control text
    /// current RGB: 51,51,51
    docker_text,
    /// Tab control text selected tab
    /// current RGB: 51,51,51
    docker_text_sel,
    /// Tab control background
    /// current RGB: 66,66,66
    docker_bg,
    /// Tab control background in windows
    /// current RGB: 120,120,120
    windowtab_bg,
    /// Envelope: Unselected automation item
    /// current RGB: 96,96,96
    auto_item_unsel,
    /// Envelope: Volume (pre-FX)
    /// current RGB: 0,220,128
    col_env1,
    /// Envelope: Volume
    /// current RGB: 0,213,27
    col_env2,
    /// Envelope: Trim Volume
    /// current RGB: 213,0,106
    env_trim_vol,
    /// Envelope: Pan (pre-FX)
    /// current RGB: 255,0,0
    col_env3,
    /// Envelope: Pan
    /// current RGB: 255,150,0
    col_env4,
    /// Envelope: Mute
    /// current RGB: 213,0,159
    env_track_mute,
    /// Envelope: Master playrate
    /// current RGB: 213,0,106
    col_env5,
    /// Envelope: Master tempo
    /// current RGB: 0,255,255
    col_env6,
    /// Envelope: Width / Send volume
    /// current RGB: 213,0,0
    col_env7,
    /// Envelope: Send pan
    /// current RGB: 0,128,128
    col_env8,
    /// Envelope: Send volume 2
    /// current RGB: 128,0,0
    col_env9,
    /// Envelope: Send pan 2
    /// current RGB: 0,128,128
    col_env10,
    /// Envelope: Send mute
    /// current RGB: 192,192,0
    env_sends_mute,
    /// Envelope: Audio hardware output volume
    /// current RGB: 0,255,255
    col_env11,
    /// Envelope: Audio hardware output pan
    /// current RGB: 255,255,0
    col_env12,
    /// Envelope: FX parameter 1
    /// current RGB: 168,0,255
    col_env13,
    /// Envelope: FX parameter 2
    /// current RGB: 48,146,147
    col_env14,
    /// Envelope: FX parameter 3
    /// current RGB: 0,130,255
    col_env15,
    /// Envelope: FX parameter 4
    /// current RGB: 192,39,69
    col_env16,
    /// Envelope: Item take volume
    /// current RGB: 128,0,0
    env_item_vol,
    /// Envelope: Item take pan
    /// current RGB: 0,128,128
    env_item_pan,
    /// Envelope: Item take mute
    /// current RGB: 192,192,0
    env_item_mute,
    /// Envelope: Item take pitch
    /// current RGB: 0,255,255
    env_item_pitch,
    /// Wiring: Background
    /// current RGB: 46,46,46
    wiring_grid2,
    /// Wiring: Background grid lines
    /// current RGB: 51,51,51
    wiring_grid,
    /// Wiring: Box border
    /// current RGB: 153,153,153
    wiring_border,
    /// Wiring: Box background
    /// current RGB: 38,38,38
    wiring_tbg,
    /// Wiring: Box foreground
    /// current RGB: 204,204,204
    wiring_ticon,
    /// Wiring: Record section background
    /// current RGB: 101,77,77
    wiring_recbg,
    /// Wiring: Record section foreground
    /// current RGB: 63,33,33
    wiring_recitem,
    /// Wiring: Media
    /// current RGB: 32,64,32
    wiring_media,
    /// Wiring: Receives
    /// current RGB: 92,92,92
    wiring_recv,
    /// Wiring: Sends
    /// current RGB: 92,92,92
    wiring_send,
    /// Wiring: Fader
    /// current RGB: 128,128,192
    wiring_fader,
    /// Wiring: Master/Parent
    /// current RGB: 64,128,128
    wiring_parent,
    /// Wiring: Master/Parent wire border
    /// current RGB: 100,100,100
    wiring_parentwire_border,
    /// Wiring: Master/Parent to master wire
    /// current RGB: 192,192,192
    wiring_parentwire_master,
    /// Wiring: Master/Parent to parent folder wire
    /// current RGB: 128,128,128
    wiring_parentwire_folder,
    /// Wiring: Pins normal
    /// current RGB: 192,192,192
    wiring_pin_normal,
    /// Wiring: Pins connected
    /// current RGB: 96,144,96
    wiring_pin_connected,
    /// Wiring: Pins disconnected
    /// current RGB: 64,32,32
    wiring_pin_disconnected,
    /// Wiring: Horizontal pin connections
    /// current RGB: 72,72,72
    wiring_horz_col,
    /// Wiring: Send hanging wire
    /// current RGB: 128,128,128
    wiring_sendwire,
    /// Wiring: Hardware output wire
    /// current RGB: 128,128,128
    wiring_hwoutwire,
    /// Wiring: Record input wire
    /// current RGB: 255,128,128
    wiring_recinputwire,
    /// Wiring: System hardware outputs
    /// current RGB: 64,64,64
    wiring_hwout,
    /// Wiring: System record inputs
    /// current RGB: 128,64,64
    wiring_recinput,
    /// Wiring: Activity lights
    /// current RGB: 64,255,64
    wiring_activity,
    /// Automatic track group
    /// current RGB: 255,255,255
    autogroup,
    /// Group #1
    /// current RGB: 255,0,0
    group_0,
    /// Group #2
    /// current RGB: 0,255,0
    group_1,
    /// Group #3
    /// current RGB: 0,0,255
    group_2,
    /// Group #4
    /// current RGB: 255,255,0
    group_3,
    /// Group #5
    /// current RGB: 255,0,255
    group_4,
    /// Group #6
    /// current RGB: 0,255,255
    group_5,
    /// Group #7
    /// current RGB: 192,0,0
    group_6,
    /// Group #8
    /// current RGB: 0,192,0
    group_7,
    /// Group #9
    /// current RGB: 0,0,192
    group_8,
    /// Group #10
    /// current RGB: 192,192,0
    group_9,
    /// Group #11
    /// current RGB: 192,0,192
    group_10,
    /// Group #12
    /// current RGB: 0,192,192
    group_11,
    /// Group #13
    /// current RGB: 128,0,0
    group_12,
    /// Group #14
    /// current RGB: 0,128,0
    group_13,
    /// Group #15
    /// current RGB: 0,0,128
    group_14,
    /// Group #16
    /// current RGB: 128,128,0
    group_15,
    /// Group #17
    /// current RGB: 128,0,128
    group_16,
    /// Group #18
    /// current RGB: 0,128,128
    group_17,
    /// Group #19
    /// current RGB: 192,128,0
    group_18,
    /// Group #20
    /// current RGB: 0,192,128
    group_19,
    /// Group #21
    /// current RGB: 0,128,192
    group_20,
    /// Group #22
    /// current RGB: 192,128,0
    group_21,
    /// Group #23
    /// current RGB: 128,0,192
    group_22,
    /// Group #24
    /// current RGB: 128,192,0
    group_23,
    /// Group #25
    /// current RGB: 64,0,0
    group_24,
    /// Group #26
    /// current RGB: 0,64,0
    group_25,
    /// Group #27
    /// current RGB: 0,0,64
    group_26,
    /// Group #28
    /// current RGB: 64,64,0
    group_27,
    /// Group #29
    /// current RGB: 64,0,64
    group_28,
    /// Group #30
    /// current RGB: 0,64,64
    group_29,
    /// Group #31
    /// current RGB: 64,0,64
    group_30,
    /// Group #32
    /// current RGB: 0,64,64
    group_31,
    /// Group #33
    /// current RGB: 128,255,255
    group_32,
    /// Group #34
    /// current RGB: 128,0,128
    group_33,
    /// Group #35
    /// current RGB: 1,255,128
    group_34,
    /// Group #36
    /// current RGB: 128,0,255
    group_35,
    /// Group #37
    /// current RGB: 1,255,255
    group_36,
    /// Group #38
    /// current RGB: 1,0,128
    group_37,
    /// Group #39
    /// current RGB: 128,255,224
    group_38,
    /// Group #40
    /// current RGB: 128,63,128
    group_39,
    /// Group #41
    /// current RGB: 32,255,128
    group_40,
    /// Group #42
    /// current RGB: 128,63,224
    group_41,
    /// Group #43
    /// current RGB: 32,255,224
    group_42,
    /// Group #44
    /// current RGB: 32,63,128
    group_43,
    /// Group #45
    /// current RGB: 128,255,192
    group_44,
    /// Group #46
    /// current RGB: 128,127,128
    group_45,
    /// Group #47
    /// current RGB: 64,255,128
    group_46,
    /// Group #48
    /// current RGB: 128,127,192
    group_47,
    /// Group #49
    /// current RGB: 64,255,192
    group_48,
    /// Group #50
    /// current RGB: 64,127,128
    group_49,
    /// Group #51
    /// current RGB: 128,127,224
    group_50,
    /// Group #52
    /// current RGB: 64,63,128
    group_51,
    /// Group #53
    /// current RGB: 32,127,128
    group_52,
    /// Group #54
    /// current RGB: 128,127,224
    group_53,
    /// Group #55
    /// current RGB: 32,255,192
    group_54,
    /// Group #56
    /// current RGB: 128,63,192
    group_55,
    /// Group #57
    /// current RGB: 128,255,160
    group_56,
    /// Group #58
    /// current RGB: 128,191,128
    group_57,
    /// Group #59
    /// current RGB: 96,255,128
    group_58,
    /// Group #60
    /// current RGB: 128,191,160
    group_59,
    /// Group #61
    /// current RGB: 96,255,160
    group_60,
    /// Group #62
    /// current RGB: 96,191,128
    group_61,
    /// Group #63
    /// current RGB: 96,255,160
    group_62,
    /// Group #64
    /// current RGB: 96,191,128
    group_63,
}
