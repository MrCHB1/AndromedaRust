local P = {}
P.plugin_name = "Rainbowify"
-- only for categorical purposes. does not affect the actual functionality of the plugin
P.plugin_type = "manipulate"
-- no params means it immediately is applied
P.dialog_fields = {}

function on_apply(notes)
    notes:for_each_selected(function (note)
        note.channel = note.key % 16
    end)
end

P.on_apply = on_apply

return P