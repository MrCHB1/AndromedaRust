local P = {}
P.plugin_name = "Flip Y (Key-wise)"
-- only for categorical purposes. does not affect the actual functionality of the plugin
P.plugin_type = "manipulate"
-- no fields AT ALL means it runs immediately
P.dialog_fields = {}

function on_apply(notes)
    local key_range = notes:get_selection_key_range(false)
    if key_range == nil then return end -- nothing selected

    local min_key = key_range.min
    local max_key = key_range.max

    notes:for_each_selected(function (note)
        note.key = max_key - note.key + min_key
    end)
end

P.on_apply = on_apply

return P