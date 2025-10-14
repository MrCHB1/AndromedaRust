local P = {}
P.plugin_name = "Flip X (Tick-wise)"
-- only for categorical purposes. does not affect the actual functionality of the plugin
P.plugin_type = "manipulate"
-- no fields AT ALL means it runs immediately
P.dialog_fields = {}

function on_apply(notes)
    local tick_range = notes:get_selection_tick_range(false)
    if tick_range == nil then return end -- nothing selected

    local min_tick = tick_range.min
    local max_tick = tick_range.max

    notes:for_each_selected(function (note)
        note.start = max_tick - note.start + min_tick
    end)
end

P.on_apply = on_apply

return P