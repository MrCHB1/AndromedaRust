local P = {}
P.plugin_name = "Mandelbrot set"
P.plugin_type = "generate"
P.dialog_fields = {}

function iter_mandelbrot(creal, cimag)
    local zreal = 0
    local zimag = 0
    local iters = 0
    for i = 1,100 do
        local ztemp = 2.0 * zreal * zimag + cimag
        zreal = zreal * zreal - zimag * zimag + creal
        zimag = ztemp

        if zreal * zreal + zimag * zimag > 4.0 then
            break
        end

        iters = iters + 1
    end

    return iters
end

function on_apply(notes)
    local ppq = andromeda:get_ppq()
    local res = 400
    local note_length = ppq*4/res
    for y = 0,127 do
        for x = 0,res do
            local creal = (x/res*2-1)*2
            local cimag = (y/127*2-1)*2
            local iters = iter_mandelbrot(creal,cimag)
            notes:create_note(note_length * x, note_length, iters % 16, y, 1)
        end
    end
end
P.on_apply = on_apply
return P