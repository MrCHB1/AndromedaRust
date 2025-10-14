local P = {}

P.plugin_name = "Example Plugin"

-- "manipulate" - this plugin does not delete or add new notes, just edits existing ones
-- "generate" - this plugin generates new notes. it can't edit existing notes
P.plugin_type = "manipulate"

-- this is optional
P.plugin_info = {
    author = "ponluxime",
    description = "This is an example plugin."
}

-- no fields AT ALL means it runs immediately
-- like this: P.dialog_fields = {}
P.dialog_fields = {
    {
        type = "label",
        label = "Hello world!"
    },
    { -- you *could* give a name to buttons and labels, but they don't contain anything readable
        type = "button",
        label = "This is a button.",
        on_click = "fn_test"
    },
    {}, -- empty field here would count as a separator in the dialog
    minmax_number = {
        type = "number",
        label = "Min/max range number thing",
        value = 1,
        range = {
            min = -10,
            max = 10
        }
    },
    nolimit_number = {
        type = "number",
        label = "Number with no limits",
        value = 47
    },
    slider_number = {
        type = "slider",
        label = "A slider!",
        value = 0.0,
        step = 0.1,
        range = {
            min = -10,
            max = 10,
        }
    },
    textfield = {
        type = "textedit",
        label = "Enter some text here",
        value = "blah blah balh abl ah"
    },
    toggleable = {
        type = "toggle",
        label = "a checkmark!",
        value = true
    },
    dropdown_thing = {
        type = "dropdown",
        label = "A dropdown",
        value = 0, -- basically the index of the current dropdown value (starts at 0 instead of 1)
        value_labels = {
            "Option A",
            "Option B",
            "Option C"
        }
    }
}

-- basic humanization for demonstration
function on_apply(notes)
    notes:for_each_selected(function (note)
        note.length = andromeda:secs_to_ticks(1.0);
    end)
end

P.on_apply = on_apply

return P