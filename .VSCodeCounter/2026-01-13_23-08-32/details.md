# Details

Date : 2026-01-13 23:08:32

Directory e:\\Programs\\Rust\\AndromedaRust

Total : 95 files,  14106 codes, 2139 comments, 2959 blanks, all 19204 lines

[Summary](results.md) / Details / [Diff Summary](diff.md) / [Diff Details](diff-details.md)

## Files
| filename | language | code | comment | blank | total |
| :--- | :--- | ---: | ---: | ---: | ---: |
| [assets/plugins/builtin/batch\_edit.lua](/assets/plugins/builtin/batch_edit.lua) | Lua | 101 | 0 | 0 | 101 |
| [assets/plugins/builtin/flip\_x.lua](/assets/plugins/builtin/flip_x.lua) | Lua | 15 | 0 | 0 | 15 |
| [assets/plugins/builtin/flip\_y.lua](/assets/plugins/builtin/flip_y.lua) | Lua | 15 | 0 | 0 | 15 |
| [assets/plugins/builtin/humanize.lua](/assets/plugins/builtin/humanize.lua) | Lua | 136 | 0 | 0 | 136 |
| [assets/plugins/custom/HZ-Chop.lua](/assets/plugins/custom/HZ-Chop.lua) | Lua | 76 | 12 | 15 | 103 |
| [assets/plugins/custom/mandelbrot.lua](/assets/plugins/custom/mandelbrot.lua) | Lua | 34 | 0 | 5 | 39 |
| [assets/plugins/custom/rainbow\_slam.lua](/assets/plugins/custom/rainbow_slam.lua) | Lua | 39 | 0 | 5 | 44 |
| [assets/plugins/custom/rainbowify.lua](/assets/plugins/custom/rainbowify.lua) | Lua | 11 | 2 | 3 | 16 |
| [assets/shaders/data\_view\_bg.frag](/assets/shaders/data_view_bg.frag) | GLSL | 27 | 0 | 9 | 36 |
| [assets/shaders/data\_view\_bg.vert](/assets/shaders/data_view_bg.vert) | GLSL | 31 | 0 | 5 | 36 |
| [assets/shaders/data\_view\_handles.frag](/assets/shaders/data_view_handles.frag) | GLSL | 19 | 0 | 6 | 25 |
| [assets/shaders/data\_view\_handles.vert](/assets/shaders/data_view_handles.vert) | GLSL | 57 | 0 | 15 | 72 |
| [assets/shaders/piano\_roll\_bg.frag](/assets/shaders/piano_roll_bg.frag) | GLSL | 35 | 0 | 11 | 46 |
| [assets/shaders/piano\_roll\_bg.vert](/assets/shaders/piano_roll_bg.vert) | GLSL | 39 | 0 | 6 | 45 |
| [assets/shaders/piano\_roll\_kb.frag](/assets/shaders/piano_roll_kb.frag) | GLSL | 20 | 0 | 7 | 27 |
| [assets/shaders/piano\_roll\_kb.vert](/assets/shaders/piano_roll_kb.vert) | GLSL | 56 | 0 | 15 | 71 |
| [assets/shaders/piano\_roll\_note.frag](/assets/shaders/piano_roll_note.frag) | GLSL | 17 | 0 | 6 | 23 |
| [assets/shaders/piano\_roll\_note.vert](/assets/shaders/piano_roll_note.vert) | GLSL | 63 | 2 | 15 | 80 |
| [assets/shaders/track\_view\_bg.frag](/assets/shaders/track_view_bg.frag) | GLSL | 31 | 0 | 10 | 41 |
| [assets/shaders/track\_view\_bg.vert](/assets/shaders/track_view_bg.vert) | GLSL | 36 | 0 | 6 | 42 |
| [assets/shaders/track\_view\_note.frag](/assets/shaders/track_view_note.frag) | GLSL | 12 | 0 | 6 | 18 |
| [assets/shaders/track\_view\_note.vert](/assets/shaders/track_view_note.vert) | GLSL | 45 | 0 | 10 | 55 |
| [build.rs](/build.rs) | Rust | 47 | 4 | 13 | 64 |
| [src/app.rs](/src/app.rs) | Rust | 7 | 0 | 0 | 7 |
| [src/app/custom\_widgets.rs](/src/app/custom_widgets.rs) | Rust | 82 | 16 | 19 | 117 |
| [src/app/main\_window.rs](/src/app/main_window.rs) | Rust | 1,908 | 151 | 355 | 2,414 |
| [src/app/rendering.rs](/src/app/rendering.rs) | Rust | 132 | 13 | 17 | 162 |
| [src/app/rendering/buffers.rs](/src/app/rendering/buffers.rs) | Rust | 187 | 23 | 26 | 236 |
| [src/app/rendering/data\_view.rs](/src/app/rendering/data_view.rs) | Rust | 452 | 37 | 108 | 597 |
| [src/app/rendering/note\_cull\_helper.rs](/src/app/rendering/note_cull_helper.rs) | Rust | 81 | 12 | 22 | 115 |
| [src/app/rendering/piano\_roll.rs](/src/app/rendering/piano_roll.rs) | Rust | 603 | 128 | 150 | 881 |
| [src/app/rendering/shaders.rs](/src/app/rendering/shaders.rs) | Rust | 66 | 3 | 13 | 82 |
| [src/app/rendering/track\_view.rs](/src/app/rendering/track_view.rs) | Rust | 363 | 22 | 90 | 475 |
| [src/app/shared.rs](/src/app/shared.rs) | Rust | 112 | 16 | 20 | 148 |
| [src/app/ui.rs](/src/app/ui.rs) | Rust | 6 | 0 | 0 | 6 |
| [src/app/ui/dialog.rs](/src/app/ui/dialog.rs) | Rust | 44 | 9 | 10 | 63 |
| [src/app/ui/dialog\_drawer.rs](/src/app/ui/dialog_drawer.rs) | Rust | 82 | 3 | 18 | 103 |
| [src/app/ui/dialog\_manager.rs](/src/app/ui/dialog_manager.rs) | Rust | 115 | 3 | 26 | 144 |
| [src/app/ui/edtior\_info.rs](/src/app/ui/edtior_info.rs) | Rust | 38 | 42 | 9 | 89 |
| [src/app/ui/main\_menu\_bar.rs](/src/app/ui/main_menu_bar.rs) | Rust | 119 | 21 | 17 | 157 |
| [src/app/ui/manual.rs](/src/app/ui/manual.rs) | Rust | 86 | 54 | 11 | 151 |
| [src/app/util.rs](/src/app/util.rs) | Rust | 2 | 0 | 0 | 2 |
| [src/app/util/image\_loader.rs](/src/app/util/image_loader.rs) | Rust | 32 | 0 | 8 | 40 |
| [src/app/util/rich\_text\_parser.rs](/src/app/util/rich_text_parser.rs) | Rust | 0 | 87 | 0 | 87 |
| [src/app/view\_settings.rs](/src/app/view_settings.rs) | Rust | 83 | 1 | 13 | 97 |
| [src/audio.rs](/src/audio.rs) | Rust | 5 | 0 | 0 | 5 |
| [src/audio/event\_playback.rs](/src/audio/event_playback.rs) | Rust | 426 | 25 | 91 | 542 |
| [src/audio/kdmapi\_engine.rs](/src/audio/kdmapi_engine.rs) | Rust | 73 | 0 | 13 | 86 |
| [src/audio/midi\_audio\_engine.rs](/src/audio/midi_audio_engine.rs) | Rust | 6 | 0 | 0 | 6 |
| [src/audio/midi\_devices.rs](/src/audio/midi_devices.rs) | Rust | 116 | 3 | 30 | 149 |
| [src/audio/track\_mixer.rs](/src/audio/track_mixer.rs) | Rust | 30 | 4 | 6 | 40 |
| [src/editor.rs](/src/editor.rs) | Rust | 12 | 0 | 0 | 12 |
| [src/editor/actions.rs](/src/editor/actions.rs) | Rust | 185 | 19 | 22 | 226 |
| [src/editor/edit\_functions.rs](/src/editor/edit_functions.rs) | Rust | 427 | 58 | 103 | 588 |
| [src/editor/editing.rs](/src/editor/editing.rs) | Rust | 101 | 5 | 21 | 127 |
| [src/editor/editing/data\_editing.rs](/src/editor/editing/data_editing.rs) | Rust | 249 | 15 | 76 | 340 |
| [src/editor/editing/lua\_note\_editing.rs](/src/editor/editing/lua_note_editing.rs) | Rust | 254 | 8 | 69 | 331 |
| [src/editor/editing/meta\_editing.rs](/src/editor/editing/meta_editing.rs) | Rust | 198 | 61 | 45 | 304 |
| [src/editor/editing/note\_editing.rs](/src/editor/editing/note_editing.rs) | Rust | 1,129 | 135 | 293 | 1,557 |
| [src/editor/editing/note\_editing/note\_sequence\_funcs.rs](/src/editor/editing/note_editing/note_sequence_funcs.rs) | Rust | 164 | 4 | 43 | 211 |
| [src/editor/editing/note\_editing\_old.rs](/src/editor/editing/note_editing_old.rs) | Rust | 1,292 | 502 | 253 | 2,047 |
| [src/editor/editing/track\_editing.rs](/src/editor/editing/track_editing.rs) | Rust | 649 | 135 | 199 | 983 |
| [src/editor/keybinds.rs](/src/editor/keybinds.rs) | Rust | 0 | 3 | 0 | 3 |
| [src/editor/midi\_bar\_cacher.rs](/src/editor/midi_bar_cacher.rs) | Rust | 78 | 4 | 18 | 100 |
| [src/editor/navigation.rs](/src/editor/navigation.rs) | Rust | 141 | 3 | 28 | 172 |
| [src/editor/playhead.rs](/src/editor/playhead.rs) | Rust | 31 | 0 | 5 | 36 |
| [src/editor/plugins.rs](/src/editor/plugins.rs) | Rust | 115 | 4 | 18 | 137 |
| [src/editor/plugins/plugin\_andromeda\_obj.rs](/src/editor/plugins/plugin_andromeda_obj.rs) | Rust | 47 | 2 | 9 | 58 |
| [src/editor/plugins/plugin\_dialog.rs](/src/editor/plugins/plugin_dialog.rs) | Rust | 402 | 73 | 56 | 531 |
| [src/editor/plugins/plugin\_error\_dialog.rs](/src/editor/plugins/plugin_error_dialog.rs) | Rust | 46 | 1 | 10 | 57 |
| [src/editor/plugins/plugin\_lua.rs](/src/editor/plugins/plugin_lua.rs) | Rust | 120 | 2 | 24 | 146 |
| [src/editor/project.rs](/src/editor/project.rs) | Rust | 81 | 5 | 23 | 109 |
| [src/editor/project/project\_data.rs](/src/editor/project/project_data.rs) | Rust | 92 | 36 | 17 | 145 |
| [src/editor/project/project\_manager.rs](/src/editor/project/project_manager.rs) | Rust | 79 | 0 | 24 | 103 |
| [src/editor/settings.rs](/src/editor/settings.rs) | Rust | 2 | 0 | 0 | 2 |
| [src/editor/settings/editor\_settings.rs](/src/editor/settings/editor_settings.rs) | Rust | 259 | 59 | 46 | 364 |
| [src/editor/settings/project\_settings.rs](/src/editor/settings/project_settings.rs) | Rust | 82 | 73 | 18 | 173 |
| [src/editor/tempo\_map.rs](/src/editor/tempo_map.rs) | Rust | 68 | 0 | 12 | 80 |
| [src/editor/util.rs](/src/editor/util.rs) | Rust | 324 | 71 | 88 | 483 |
| [src/main.rs](/src/main.rs) | Rust | 26 | 19 | 5 | 50 |
| [src/midi.rs](/src/midi.rs) | Rust | 5 | 0 | 0 | 5 |
| [src/midi/events.rs](/src/midi/events.rs) | Rust | 4 | 0 | 0 | 4 |
| [src/midi/events/channel\_event.rs](/src/midi/events/channel_event.rs) | Rust | 18 | 0 | 2 | 20 |
| [src/midi/events/mergers.rs](/src/midi/events/mergers.rs) | Rust | 49 | 1 | 7 | 57 |
| [src/midi/events/meta\_event.rs](/src/midi/events/meta_event.rs) | Rust | 55 | 0 | 4 | 59 |
| [src/midi/events/note.rs](/src/midi/events/note.rs) | Rust | 72 | 2 | 16 | 90 |
| [src/midi/io.rs](/src/midi/io.rs) | Rust | 1 | 0 | 0 | 1 |
| [src/midi/io/buffered\_reader.rs](/src/midi/io/buffered_reader.rs) | Rust | 107 | 5 | 25 | 137 |
| [src/midi/midi\_file.rs](/src/midi/midi_file.rs) | Rust | 359 | 86 | 78 | 523 |
| [src/midi/midi\_track.rs](/src/midi/midi_track.rs) | Rust | 59 | 0 | 11 | 70 |
| [src/midi/midi\_track\_parser.rs](/src/midi/midi_track_parser.rs) | Rust | 351 | 26 | 26 | 403 |
| [src/util.rs](/src/util.rs) | Rust | 3 | 0 | 0 | 3 |
| [src/util/expression\_parser.rs](/src/util/expression_parser.rs) | Rust | 28 | 28 | 5 | 61 |
| [src/util/system\_stats.rs](/src/util/system_stats.rs) | Rust | 96 | 1 | 15 | 112 |
| [src/util/timer.rs](/src/util/timer.rs) | Rust | 28 | 0 | 5 | 33 |

[Summary](results.md) / Details / [Diff Summary](diff.md) / [Diff Details](diff-details.md)