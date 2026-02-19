function dump(name, t, indent)
    indent = indent or ""
    print(indent .. "--- Dump: " .. name .. " ---")
    for k, v in pairs(t) do
        print(indent .. tostring(k) .. ": " .. tostring(v))
    end
end

if net then
    dump("net library", net)
else
    print("Error: 'net' library not found")
    return
end

local s = net.socket("udp")
if s then
    local mt = getmetatable(s)
    if mt then
        dump("socket methods (via metatable.__index)", mt.__index or mt, "  ")
    else
        print("Error: could not get metatable for socket")
    end
    s:close()
else
    print("Error: net.socket() failed")
end
