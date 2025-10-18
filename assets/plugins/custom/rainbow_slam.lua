local P = {}

P.plugin_name = "Generate Rainbow Slam"
P.plugin_type = "generate"

P.dialog_fields = {
    slam_width = {
        type = "number",
        label = "Slam Width",
        value = 16,
        range = {
            min = 1,
            max = 128
        }
    },
    rainbow_spacing = {
        type = "number",
        label = "Rainbow Spacing",
        value = 1,
        range = {
            min = 1,
            max = 16
        }
    }
}

function on_apply(notes) 
    local ppq = andromeda:get_ppq()
    for i = 1, P.dialog_fields.slam_width.value do
        local note_key = i - 1
        local note_channel = ((i - 1) / P.dialog_fields.rainbow_spacing.value) % 16
        notes:create_note(0, ppq, note_channel, note_key, 127)
    end
end

P.on_apply = on_apply

return P