local P = {}

P.plugin_name = "Generate Rainbow Slam"
P.plugin_type = "generate"

P.dialog_fields = {
    {
        id = "slam_width",
        {
            type = "number",
            label = "Slam Width",
            value = 16,
            range = {
                min = 1,
                max = 128
            }
        }
    },
    {
        id = "rainbow_spacing",
        {
            type = "number",
            label = "Rainbow Spacing",
            value = 1,
            range = {
                min = 1,
                max = 16
            }
        }
    }
}

function on_apply(notes) 
    local ppq = andromeda:get_ppq()
    for i = 1, get_field_value("slam_width") do
        local note_key = i - 1
        local note_channel = ((i - 1) / get_field_value("rainbow_spacing")) % 16
        notes:create_note(0, ppq, note_channel, note_key, 127)
    end
end

P.on_apply = on_apply

return P