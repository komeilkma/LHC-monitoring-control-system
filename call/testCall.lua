-- @author komeilkma
-- @module call.testCall
module(...,package.seeall)
require"cc"
require"audio"
require"common"
local coIncoming

local function callVolTest()
    local curVol = audio.getCallVolume()
    curVol = (curVol>=7) and 1 or (curVol+1)
    log.info("testCall.setCallVolume",curVol)
    audio.setCallVolume(curVol)
end

local function testAudioStream(streamType)
    sys.taskInit(
        function()
            while true do
                tStreamType = streamType
		    	log.info("AudioTest.AudioStreamTest", "AudioStreamPlay Start", streamType)
                local tAudioFile =
                {
                  --  [audiocore.AMR] = "tip.amr",
                    [audiocore.SPX] = "record.spx",
                    [audiocore.PCM] = "alarm_door.pcm",
                    [audiocore.MP3] = "sms.mp3"
                }
                local fileHandle = io.open("/lua/" .. tAudioFile[streamType], "rb")
                if not fileHandle then
                    log.error("AudioTest.AudioStreamTest", "Open file error")
                    return
                end

                while true do
                    local data = fileHandle:read(streamType == audiocore.SPX and 1200 or 1024)
                    if not data then 
		    			fileHandle:close() 
                        while audiocore.streamremain() ~= 0 do
                            sys.wait(20)	
                        end
                        sys.wait(1000)
                        audiocore.stop()
                        log.warn("AudioTest.AudioStreamTest", "AudioStreamPlay Over")
                        return 
		    		end

                    local data_len = string.len(data)
                    local curr_len = 1
                    while true do
                        curr_len = curr_len + audiocore.streamplay(tStreamType,string.sub(data,curr_len,-1),audiocore.PLAY_VOLTE)
                        if curr_len>=data_len then
                            break
                        elseif curr_len == 0 then
                            log.error("AudioTest.AudioStreamTest", "AudioStreamPlay Error", streamType)
                            return
                        end
                        sys.wait(10)
                    end
                    sys.wait(10)
                end  
            end
        end
    )
end

local function connected(num)
    log.info("testCall.connected")
    coIncoming = nil
    sys.timerLoopStart(callVolTest,5000)
    audio.play(7,"TTS","In-call TTS test",7,nil,true,2000)
    sys.timerStart(cc.hangUp,110000,num)
end
local function disconnected(discReason)
    coIncoming = nil
    log.info("testCall.disconnected",discReason)
    sys.timerStopAll(cc.hangUp)
    sys.timerStop(callVolTest)
    audio.stop()
end

local function incoming(num)
    log.info("testCall.incoming:"..num)
    
    if not coIncoming then
        coIncoming = sys.taskInit(function()
            while true do
                audio.play(1,"FILE","/lua/call.mp3",4,function() sys.publish("PLAY_INCOMING_RING_IND") end,true)
                sys.waitUntil("PLAY_INCOMING_RING_IND")
                break                
            end
        end)
        sys.subscribe("POWER_KEY_IND",function() audio.stop(function() cc.accept(num) end) end)
    end
    
    --[[
    if not coIncoming then
        coIncoming = sys.taskInit(function()
            for i=1,7 do
                --audio.play(1,"TTS","In-call TTS test",i,function() sys.publish("PLAY_INCOMING_RING_IND") end)
                audio.play(1,"FILE","/lua/call.mp3",i,function() sys.publish("PLAY_INCOMING_RING_IND") end)
                sys.waitUntil("PLAY_INCOMING_RING_IND")
            end
            --cc.accept(num)
        end)      
    end]]
    --cc.accept(num)   
end

local function ready()
    log.info("tesCall.ready")
end

local function dtmfDetected(dtmf)
    log.info("testCall.dtmfDetected",dtmf)
end

sys.subscribe("NET_STATE_REGISTERED",ready)
sys.subscribe("CALL_INCOMING",incoming)
sys.subscribe("CALL_CONNECTED",connected)
sys.subscribe("CALL_DISCONNECTED",disconnected)
cc.dtmfDetect(true)
sys.subscribe("CALL_DTMF_DETECT",dtmfDetected)



