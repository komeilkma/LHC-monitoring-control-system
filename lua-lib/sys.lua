-- @module sys
-- @author komeilkma
module(..., package.seeall)
SCRIPT_LIB_VER = "1.0.0"
local TASK_TIMER_ID_MAX = 0x1FFFFFFF
local MSG_TIMER_ID_MAX = 0x7FFFFFFF
local taskTimerId = 0
local msgId = TASK_TIMER_ID_MAX
local timerPool = {}
local taskTimerPool = {}
local para = {}
local loop = {}



-- @usage sys.powerOn()
function powerOn()
    rtos.poweron(1)
end
function restart(r)
    assert(r and r ~= "", "sys.restart cause null")
    if errDump and errDump.appendErr and type(errDump.appendErr) == "function" then errDump.appendErr("restart[" .. r .. "];") end
    log.warn("sys.restart", r)
    rtos.restart()
end
function wait(ms)

    assert(ms > 0, "The wait time cannot be negative!")

    if ms < 5 then ms = 5 end

    if taskTimerId >= TASK_TIMER_ID_MAX then taskTimerId = 0 end
    taskTimerId = taskTimerId + 1
    local timerid = taskTimerId
    taskTimerPool[coroutine.running()] = timerid
    timerPool[timerid] = coroutine.running()
    if 1 ~= rtos.timer_start(timerid, ms) then log.debug("rtos.timer_start error") return end
    local message = {coroutine.yield()}
    if #message ~= 0 then
        rtos.timer_stop(timerid)
        taskTimerPool[coroutine.running()] = nil
        timerPool[timerid] = nil
        return unpack(message)
    end
end



function waitUntil(id, ms)
    subscribe(id, coroutine.running())
    local message = ms and {wait(ms)} or {coroutine.yield()}
    unsubscribe(id, coroutine.running())
    return message[1] ~= nil, unpack(message, 2, #message)
end


function waitUntilExt(id, ms)
    subscribe(id, coroutine.running())
    local message = ms and {wait(ms)} or {coroutine.yield()}
    unsubscribe(id, coroutine.running())
    if message[1] ~= nil then return unpack(message) end
    return false
end

-- @usage sys.taskInit(task1,'a','b')
function taskInit(fun, ...)
    local co = coroutine.create(fun)
    coroutine.resume(co, ...)
    return co
end

-- @usage sys.init(1,0)
function init(mode, lprfnc)
    assert(PROJECT and PROJECT ~= "" and VERSION and VERSION ~= "", "Undefine PROJECT or VERSION")
    collectgarbage("setpause", 80)

    uart.setup(uart.ATC, 0, 0, uart.PAR_NONE, uart.STOP_1)
    log.info("poweron reason:", rtos.poweron_reason(), PROJECT, VERSION, SCRIPT_LIB_VER, rtos.get_version())
    pcall(rtos.set_lua_info,"\r\n"..rtos.get_version().."\r\n"..(_G.PROJECT or "NO PROJECT").."\r\n"..(_G.VERSION or "NO VERSION"))
    if type(rtos.get_build_time)=="function" then log.info("core build time", rtos.get_build_time()) end
    if mode == 1 then
        if rtos.poweron_reason() == rtos.POWERON_CHARGER then
            rtos.poweron(0)
        end
    end
end

local function cmpTable(t1, t2)
    if not t2 then return #t1 == 0 end
    if #t1 == #t2 then
        for i = 1, #t1 do
            if unpack(t1, i, i) ~= unpack(t2, i, i) then
                return false
            end
        end
        return true
    end
    return false
end

-- sys.timerStart(publicTimerCbFnc,8000,"first")
-- sys.timerStop(publicTimerCbFnc,"first")
function timerStop(val, ...)
	local arg={ ... }
    if type(val) == 'number' then
        timerPool[val], para[val], loop[val] = nil
        rtos.timer_stop(val)
    else
        for k, v in pairs(timerPool) do
            if type(v) == 'table' and v.cb == val or v == val then
                if cmpTable(arg, para[k]) then
                    rtos.timer_stop(k)
                    timerPool[k], para[k], loop[val] = nil
                    break
                end
            end
        end
    end
end

function timerStopAll(fnc)
    for k, v in pairs(timerPool) do
        if type(v) == "table" and v.cb == fnc or v == fnc then
            rtos.timer_stop(k)
            timerPool[k], para[k], loop[k] = nil
        end
    end
end

-- sys.timerStart(function(tag) log.info("timerCb",tag) end, 5000, "test")
function timerStart(fnc, ms, ...)
	local arg={ ... }
	local argcnt=0
	for i, v in pairs(arg) do
		argcnt = argcnt+1
	end
    assert(fnc ~= nil, "sys.timerStart(first param) is nil !")
    assert(ms > 0, "sys.timerStart(Second parameter) is <= zero !")
    if ms < 5 then ms = 5 end
    if argcnt == 0 then
        timerStop(fnc)
    else
        timerStop(fnc, ...)
    end
    while true do
        if msgId >= MSG_TIMER_ID_MAX then msgId = TASK_TIMER_ID_MAX end
        msgId = msgId + 1
        if timerPool[msgId] == nil then
            timerPool[msgId] = fnc
            break
        end
    end
    if rtos.timer_start(msgId, ms) ~= 1 then log.debug("rtos.timer_start error") return end
    if argcnt ~= 0 then
        para[msgId] = arg
    end
    return msgId
end

-- sys.timerLoopStart(function(tag) log.info("timerCb",tag) end, 5000, "test")
function timerLoopStart(fnc, ms, ...)
    local tid = timerStart(fnc, ms, ...)
    if tid then loop[tid] = (ms<5 and 5 or ms) end
    return tid
end

function timerIsActive(val, ...)
	local arg={ ... }
    if type(val) == "number" then
        return timerPool[val]
    else
        for k, v in pairs(timerPool) do
            if v == val then
                if cmpTable(arg, para[k]) then return true end
            end
        end
    end
end


local subscribers = {}
local messageQueue = {}

-- @usage subscribe("NET_STATUS_IND", callback)
function subscribe(id, callback)
    if type(id) ~= "string" or (type(callback) ~= "function" and type(callback) ~= "thread") then
        log.warn("warning: sys.subscribe invalid parameter", id, callback)
        return
    end
    if not subscribers[id] then subscribers[id] = {count = 0} end
    if not subscribers[id][callback] then
        subscribers[id].count = subscribers[id].count + 1
        subscribers[id][callback] = true
    end
end

-- @usage unsubscribe("NET_STATUS_IND", callback)
function unsubscribe(id, callback)
    if type(id) ~= "string" or (type(callback) ~= "function" and type(callback) ~= "thread") then
        log.warn("warning: sys.unsubscribe invalid parameter", id, callback)
        return
    end
    if subscribers[id] then
        if subscribers[id][callback] then
            subscribers[id].count = subscribers[id].count - 1
            subscribers[id][callback] = false
        end
    end
end

-- @usage publish("NET_STATUS_IND")
function publish(...)
	local arg = { ... }
    table.insert(messageQueue, arg)
end

local function dispatch()
    while true do
        if #messageQueue == 0 then
            for k, v in pairs(subscribers) do
                if v.count == 0 then subscribers[k] = nil end
            end
            break
        end
        local message = table.remove(messageQueue, 1)
        if subscribers[message[1]] then
            for callback, flag in pairs(subscribers[message[1]]) do
                if flag then
                    if type(callback) == "function" then
                        callback(unpack(message, 2, #message))
                    elseif type(callback) == "thread" then
                        coroutine.resume(callback, unpack(message))
                    end
                end
            end
            if subscribers[message[1]] then
                for callback, flag in pairs(subscribers[message[1]]) do
                    if not flag then
                        subscribers[message[1]][callback] = nil
                    end
                end
            
                if subscribers[message[1]].count == 0 then
                    subscribers[message[1]] = nil
                end
            end
        end
    end
end


local handlers = {}
setmetatable(handlers, {__index = function() return function() end end, })

rtos.on = function(id, handler)
    handlers[id] = handler
end


function run()
    while true do
        dispatch()
        local msg, param = rtos.receive(rtos.INF_TIMEOUT)
        if msg == rtos.MSG_TIMER and timerPool[param] then
            if param <= TASK_TIMER_ID_MAX then
                local taskId = timerPool[param]
                timerPool[param] = nil
                if taskTimerPool[taskId] == param then
                    taskTimerPool[taskId] = nil
                    coroutine.resume(taskId)
                end
            else
                local cb = timerPool[param]
                if not loop[param] then timerPool[param] = nil end
                if para[param] ~= nil then
                    cb(unpack(para[param]))
                    if not loop[param] then para[param] = nil end
                else
                    cb()
                end
                if loop[param] then rtos.timer_start(param, loop[param]) end
            end
        elseif type(msg) == "number" then
            handlers[msg](param)
        else
            handlers[msg.id](msg)
        end
    end
end

require "clib"

if type(rtos.openSoftDog)=="function" then
    rtos.openSoftDog(60000)
    sys.timerLoopStart(rtos.eatSoftDog,20000)
end
