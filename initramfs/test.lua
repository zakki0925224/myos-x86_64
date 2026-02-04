local f = io.open("/dev/zakki", "r")
if f then
    print("Content: " .. f:read("*a"))
    f:close()
end
