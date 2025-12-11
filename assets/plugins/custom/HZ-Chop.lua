local P = {}

--Set up
local HZ_Map = {}

for i = 0 , 127 do 
    HZ_Map[i] = 440 * math.pow(2, (i - 69) / 12.0) 
end


--Plugin info
P.plugin_name = "HZ-Chop"
P.plugin_type = "generate"
P.dialog_fields = {
    {
        id = "iNote",
        {
            type = "number",
            label = "The key you want to chop your notes to. Input -1 to use the selected notes key",
            value = -1,
            range = {
            min = -1,
            max = 127
            }
        }
    }
}
P.plugin_info = {
    author = "Softy107",
    description = "HZ-Chop the selected notes."
}

--Main function
function on_apply(notes)
    local PPQ = andromeda:get_ppq()
    local selected_notes = {}
    local curTick
    
    notes:for_each_selected(function (note)  
        local Hz
	local NoteValue = get_field_value("iNote")
        --Select between dynamic and static chopping
        if NoteValue == -1 then
            Hz = HZ_Map[note.key]
        elseif NoteValue >= 0 and NoteValue <= 127 then
            Hz = HZ_Map[NoteValue]
        else -- Edge case just because
            print("Invalid value for key, please select a number between or equal to -1 and 127")
        end

	--Calculations
	curTick = note.start
        local BPS = 1 / andromeda:ticks_to_secs(PPQ)
        local BaseGate = (BPS * PPQ) / Hz
	
	local frac = BaseGate - math.floor(BaseGate)
	local fracAcc = 0.0

	--Add notes for placing
        while curTick < note.start + note.length do   
	    local NoteGate

	    fracAcc = fracAcc + frac
    	    if fracAcc >= 1 then
       	        NoteGate = math.ceil(BaseGate)
                fracAcc = fracAcc - 1
    	    else
        	NoteGate = math.floor(BaseGate)
    	    end
		print(BaseGate, NoteGate, fracAcc)
         
	    table.insert(selected_notes, {
                start = curTick,
            	length = NoteGate,
            	channel = note.channel,
            	key = note.key,
            	velocity = note.velocity
            })
            curTick = curTick + NoteGate
        end
    end)

    --Note placing
    for _, note in ipairs(selected_notes) do
        notes:create_note(
            note.start,
            note.length,
            note.channel,
            note.key,
            note.velocity
        )
    end
end

-----------------------------------------
--Fun Fact of the version :
--I like to snack on RAM sticks late at night
--DDR4 is my regular, though I do take a DDR5 stick if I'm feeling fancy
-----------------------------------------

P.on_apply = on_apply
return P
