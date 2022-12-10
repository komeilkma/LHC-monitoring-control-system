-- @module utils
-- @author komeilkma
module(..., package.seeall)

function string.toHex(str, separator)
    return str:gsub('.', function(c)
        return string.format("%02X" .. (separator or ""), string.byte(c))
    end)
end
function string.fromHex(hex)
    local hex = hex:gsub("[%s%p]", ""):upper()
    return hex:gsub("%x%x", function(c)
        return string.char(tonumber(c, 16))
    end)
end

function string.toValue(str)
    return string.fromHex(str:gsub("%x", "0%1"))
end

function string.utf8Len(str)
    local _, count = string.gsub(str, "[^\128-\193]", "")
    return count
end


function string.utf8ToTable(str)
    local tab = {}
    for uchar in string.gfind(str, "[%z\1-\127\194-\244][\128-\191]*") do
        tab[#tab + 1] = uchar
    end
    return tab
end

function string.rawurlEncode(str)
    local t = str:utf8ToTable()
    for i = 1, #t do
        if #t[i] == 1 then
            t[i] = string.gsub(string.gsub(t[i], "([^%w_%~%.%- ])", function(c) return string.format("%%%02X", string.byte(c)) end), " ", "%%20")
        else
            t[i] = string.gsub(t[i], ".", function(c) return string.format("%%%02X", string.byte(c)) end)
        end
    end
    return table.concat(t)
end

function string.urlEncode(str)
    local t = str:utf8ToTable()
    for i = 1, #t do
        if #t[i] == 1 then
            t[i] = string.gsub(string.gsub(t[i], "([^%w_%*%.%- ])", function(c) return string.format("%%%02X", string.byte(c)) end), " ", "+")
        else
            t[i] = string.gsub(t[i], ".", function(c) return string.format("%%%02X", string.byte(c)) end)
        end
    end
    return table.concat(t)
end

function table.gsort(t, f)
    local a = {}
    for n in pairs(t) do a[#a + 1] = n end
    table.sort(a, f)
    local i = 0
    return function()
        i = i + 1
        return a[i], t[a[i]]
    end
end

function table.rconcat(l)
    if type(l) ~= "table" then return l end
    local res = {}
    for i = 1, #l do
        res[i] =table.rconcat(l[i])
    end
    return table.concat(res)
end

function string.formatNumberThousands(num)
    local k, formatted
    formatted = tostring(tonumber(num))
    while true do
        formatted, k = string.gsub(formatted, "^(-?%d+)(%d%d%d)", '%1,%2')
        if k == 0 then break end
    end
    return formatted
end

function string.split(str, delimiter)
    local strlist, tmp = {}, string.byte(delimiter)
    if delimiter == "" then
        for i = 1, #str do strlist[i] = str:sub(i, i) end
    else
        for substr in string.gmatch(str .. delimiter, "(.-)" .. (((tmp > 96 and tmp < 123) or (tmp > 64 and tmp < 91) or (tmp > 47 and tmp < 58)) and delimiter or "%" .. delimiter)) do
            table.insert(strlist, substr)
        end
    end
    return strlist
end

function string.checkSum(str, num)
    assert(type(str) == "string", "The first argument is not a string!")
    local sum = 0
    for i = 1, #str do
        sum = sum + str:sub(i, i):byte()
    end
    if num == 2 then
        return sum % 0x10000
    else
        return sum % 0x100
    end
end

function io.exists(path)
    local file = io.open(path, "r")
    if file then
        io.close(file)
        return true
    end
    return false
end

function io.readFile(path)
    local file = io.open(path, "rb")
    if file then
        local content = file:read("*a")
        io.close(file)
        return content
    end
end

function io.writeFile(path, content, mode)
    local mode = mode or "w+b"
    local file = io.open(path, mode)
    if file then
        if file:write(content) == nil then return false end
        io.close(file)
        return true
    else
        return false
    end
end

function io.pathInfo(path)
    local pos = string.len(path)
    local extpos = pos + 1
    while pos > 0 do
        local b = string.byte(path, pos)
        if b == 46 then -- 46 = char "."
            extpos = pos
        elseif b == 47 then -- 47 = char "/"
            break
        end
        pos = pos - 1
    end
    
    local dirname = string.sub(path, 1, pos)
    local filename = string.sub(path, pos + 1)
    extpos = extpos - pos
    local basename = string.sub(filename, 1, extpos - 1)
    local extname = string.sub(filename, extpos)
    return {
        dirname = dirname,
        filename = filename,
        basename = basename,
        extname = extname
    }
end

function io.fileSize(path)
    local size = 0
    local file = io.open(path, "r")
    if file then
        local current = file:seek()
        size = file:seek("end")
        file:seek("set", current)
        io.close(file)
    end
    return size
end

function io.readStream(path, offset, len)
    local file, str = io.open(path, "r")
    if file then
        local current = file:seek()
        file:seek("set", offset)
        str = file:read(len)
        file:seek("set", current)
        io.close(file)
    end
    return str
end
