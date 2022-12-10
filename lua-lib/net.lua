-- @module net
-- @author komeilkma
require "sys"
require "utils"
module(..., package.seeall)

local publish = sys.publish

--netmode define
NetMode_noNet=   0
NetMode_GSM=     1--2G
NetMode_EDGE=    2--2.5G
NetMode_TD=      3--3G
NetMode_LTE=     4--4G
NetMode_WCDMA=   5--3G
local netMode = NetMode_noNet

local state = "INIT"
local simerrsta
flyMode = false

local lac, ci, rssi, rsrp, band = "", "", 0, 0, ""


local cellinfo, multicellcb = {}
local curCellSeted

local function cops(data)
    local fmt,oper = data:match('COPS:%s*%d+%s*,(%d+)%s*,"(%d+)"')
    log.info("cops",fmt,oper,curCellSeted)
    if fmt=="2" and not curCellSeted then
        cellinfo[1].mcc = tonumber(oper:sub(1,3),16)
        cellinfo[1].mnc = tonumber(oper:sub(4,5),16)
    end
end

local function creg(data)
    local p1, s,act
    local prefix = (netMode == NetMode_LTE) and "+CEREG: " or (netMode == NetMode_noNet and "+CREG: " or "+CGREG: ")
    log.info("net.creg1",netMode,prefix)
    if not data:match(prefix) then
        --log.info("net.creg2",prefix)
        if prefix=="+CREG: " then
            --log.info("net.creg3")
            prefix = "+CGREG: "
            if not data:match("+CGREG: ") then
                log.warn("net.creg1","no match",data)
                return
            end
        elseif prefix=="+CGREG: " then
            --log.info("net.creg4")
            prefix = "+CREG: "
            if not data:match("+CREG: ") then
                log.warn("net.creg2","no match",data)
                return
            end
        end        
    end
    _, _, p1 = data:find(prefix .. "%d,(%d+)")
    if p1 == nil then
        _, _, p1 = data:find(prefix .. "(%d+)")
        if p1 == nil then return end
        act = data:match(prefix .. "%d+,.-,.-,(%d+)")
    else
        act = data:match(prefix .. "%d,%d+,.-,.-,(%d+)")
    end
    
    log.info("net.creg7",p1,act)

    s = (p1=="1" or p1=="5") and "REGISTERED" or "UNREGISTER"
    
    if prefix=="+CGREG: " and s=="UNREGISTER" then
        log.info("net.creg9 ignore!!!")
        return
    end
    if s ~= state then
        if s == "REGISTERED" then
            publish("NET_STATE_REGISTERED")
            cengQueryPoll()
        end
        state = s
    end
    if state == "REGISTERED" then
        p2, p3 = data:match("\"(%x+)\",\"(%x+)\"")
        if p2 and p3 and (lac ~= p2 or ci ~= p3) then
            lac = p2
            ci = p3
            publish("NET_CELL_CHANGED")
            --cellinfo[1].mcc = tonumber(sim.getMcc(),16)
            --cellinfo[1].mnc = tonumber(sim.getMnc(),16)
            cellinfo[1].lac = tonumber(lac,16)
            cellinfo[1].ci = tonumber(ci,16)
            cellinfo[1].rssi = 28
        end

        if act then
            if act=="0" then
                UpdNetMode("^MODE: 3,1")
            elseif act=="1" then
                UpdNetMode("^MODE: 3,2")
            elseif act=="3" then
                UpdNetMode("^MODE: 3,3")
            elseif act=="7" then
                UpdNetMode("^MODE: 17,17")
            else
                UpdNetMode("^MODE: 5,7")
            end
        end
    end
end

local function resetCellInfo()
    local i
    cellinfo.cnt = 11
    for i = 1, cellinfo.cnt do
        cellinfo[i] = {}
        cellinfo[i].mcc, cellinfo[i].mnc = nil
        cellinfo[i].lac = 0
        cellinfo[i].ci = 0
        cellinfo[i].rssi = 0
        cellinfo[i].ta = 0
    end
end

local function eemLteSvc(data)
    local mcc,mnc,lac,ci,rssi,svcData
    if data:match("%+EEMLTESVC:%s*%d+,%s*%d+,%s*%d+,%s*.+") then
        svcData = string.match(data, "%+EEMLTESVC:(.+)")
        if svcData then
            svcDataT = string.split(svcData, ', ')
            if not(svcDataT[1] and svcDataT[3] and svcDataT[4] and svcDataT[10] and svcDataT[15]) then
                svcDataT = string.split(svcData, ',')
                log.info("eemLteSvc2",svcDataT[1],svcDataT[3],svcDataT[4],svcDataT[10],svcDataT[15])
            end
            mcc = svcDataT[1]
            mnc = svcDataT[3]
            lac = svcDataT[4]
            ci = svcDataT[10]
			band = svcDataT[8]
            rssi = (tonumber(svcDataT[15])-(tonumber(svcDataT[15])%3))/3
            if rssi>31 then rssi=31 end
            if rssi<0 then rssi=0 end
        end
        log.info("eemLteSvc1",lac,ci,mcc,mnc)
        if lac and lac~="0" and ci and ci ~= "0" and mcc and mnc then
            resetCellInfo()
            curCellSeted = true
            cellinfo[1].mcc = mcc
            cellinfo[1].mnc = mnc
            cellinfo[1].lac = tonumber(lac)
            cellinfo[1].ci = tonumber(ci)
            cellinfo[1].rssi = tonumber(rssi)
            if multicellcb then multicellcb(cellinfo) end
            publish("CELL_INFO_IND", cellinfo)
        end
    elseif data:match("%+EEMLTEINTER") or data:match("%+EEMLTEINTRA") or data:match("%+EEMLTEINTERRAT") then
        data = data:gsub(" ","")

        if data:match("%+EEMLTEINTERRAT") then
            mcc,mnc,lac,ci,rssi = data:match("[-]*%d+,[-]*%d+,([-]*%d+),([-]*%d+),([-]*%d+),([-]*%d+),[-]*%d+,[-]*%d+,([-]*%d+)")
        else
            rssi,mcc,mnc,lac,ci = data:match("[-]*%d+,[-]*%d+,[-]*%d+,([-]*%d+),[-]*%d+,([-]*%d+),([-]*%d+),([-]*%d+),([-]*%d+)")
        end
        if rssi then
            rssi = (rssi-(rssi%3))/3
            if rssi>31 then rssi=31 end
            if rssi<0 then rssi=0 end
        end
        if lac~="0" and lac~="-1" and ci~="0" and ci~="-1" then
            for i = 1, cellinfo.cnt do
                if cellinfo[i].lac==0 then
                    cellinfo[i] = 
                    {
                        mcc = mcc,
                        mnc = mnc,
                        lac = tonumber(lac),
                        ci = tonumber(ci),
                        rssi = tonumber(rssi)
                    }
                    break
                end
            end
        end
    end
end

local function eemGsmInfoSvc(data)
	if string.find(data, "%+EEMGINFOSVC:%s*%d+,%s*%d+,%s*%d+,%s*.+") then
		local mcc,mnc,lac,ci,ta,rssi
		local svcData = string.match(data, "%+EEMGINFOSVC:(.+)")
		if svcData then
			svcDataT = string.split(svcData, ', ')
			mcc = svcDataT[1]
			mnc = svcDataT[2]
			lac = svcDataT[3]
			ci = svcDataT[4]
			ta = svcDataT[10]
			rssi = svcDataT[12]
			if tonumber(rssi) >31
				then rssi = 31
			end
			if tonumber(rssi) < 0
				then rssi = 0
			end
		end
		if lac and lac~="0" and ci and ci ~= "0" and mcc and mnc then
			resetCellInfo()
         curCellSeted = true
			cellinfo[1].mcc = mcc
			cellinfo[1].mnc = mnc
			cellinfo[1].lac = tonumber(lac)
			cellinfo[1].ci = tonumber(ci)
			cellinfo[1].rssi = (tonumber(rssi) == 99) and 0 or tonumber(rssi)
			cellinfo[1].ta = tonumber(ta or "0")
			if multicellcb then multicellcb(cellinfo) end
			publish("CELL_INFO_IND", cellinfo)
		end
	end
end

local function eemGsmNCInfoSvc(data)
	if string.find(data, "%+EEMGINFONC: %d+, %d+, %d+, .+") then
		local mcc,mnc,lac,ci,ta,rssi,id
		local svcData = string.match(data, "%+EEMGINFONC:(.+)")
		if svcData then
			svcDataT = string.split(svcData, ', ')
			id = svcDataT[1]
			mcc = svcDataT[2]
			mnc = svcDataT[3]
			lac = svcDataT[4]
			ci = svcDataT[6]
			rssi = svcDataT[7]
			if tonumber(rssi) >31
				then rssi = 31
			end
			if tonumber(rssi) < 0
				then rssi = 0
			end
		end
		if lac and ci and mcc and mnc then
			cellinfo[id + 2].mcc = mcc
			cellinfo[id + 2].mnc = mnc
			cellinfo[id + 2].lac = tonumber(lac)
			cellinfo[id + 2].ci = tonumber(ci)
			cellinfo[id + 2].rssi = (tonumber(rssi) == 99) and 0 or tonumber(rssi)
		end
	end
end

local function eemUMTSInfoSvc(data)
	if string.find(data, "%+EEMUMTSSVC: %d+, %d+, %d+, .+") then
		local mcc,mnc,lac,ci,rssi
		local svcData = string.match(data, "%+EEMUMTSSVC:(.+)")
		local cellMeasureFlag, cellParamFlag = string.match(data, "%+EEMUMTSSVC:%d+, (%d+), (%d+), .+")
		local svcDataT = string.split(svcData, ', ')
		local offset = 4
		if svcData and svcDataT then
			if tonumber(cellMeasureFlag) ~= 0 then
				offset = offset + 2
				rssi = svcDataT[offset]
				offset = offset + 4
			else 
				offset = offset + 2
				rssi = svcDataT[offset]
				offset = offset + 2
			end

			if tonumber(cellParamFlag) ~= 0 then
				offset = offset + 3
				mcc = svcDataT[offset]
				mnc = svcDataT[offset + 1]
				lac = svcDataT[offset + 2]
				ci  = svcDataT[offset + 3]
				offset = offset + 3
			end
		end
		if lac and lac~="0" and ci and ci ~= "0" and mcc and mnc and rssi then
			resetCellInfo()
         curCellSeted = true   
			cellinfo[1].mcc = mcc
			cellinfo[1].mnc = mnc
			cellinfo[1].lac = tonumber(lac)
			cellinfo[1].ci = tonumber(ci)
			cellinfo[1].rssi = tonumber(rssi)
			if multicellcb then multicellcb(cellinfo) end
			publish("CELL_INFO_IND", cellinfo)
		end
	end
end

function UpdNetMode(data)
	local _, _, SysMainMode,SysMode = string.find(data, "(%d+),(%d+)")
	local netMode_cur
	log.info("net.UpdNetMode",netMode_cur,netMode, SysMainMode,SysMode)
	if SysMainMode and SysMode then
		if SysMainMode=="3" then
			netMode_cur = NetMode_GSM
		elseif SysMainMode=="5" then
			netMode_cur = NetMode_WCDMA
		elseif SysMainMode=="15" then
			netMode_cur = NetMode_TD
		elseif SysMainMode=="17" then
			netMode_cur = NetMode_LTE
		else
			netMode_cur = NetMode_noNet
		end
		
		if SysMode=="3" then
			netMode_cur = NetMode_EDGE
		end
	end
  
	if netMode ~= netMode_cur then
		netMode = netMode_cur
		publish("NET_UPD_NET_MODE",netMode)
		log.info("net.NET_UPD_NET_MODE",netMode)   
		ril.request("AT+COPS?")
		if netMode == NetMode_LTE then 
			ril.request("AT+CEREG?")  
		elseif netMode == NetMode_noNet then 
			ril.request("AT+CREG?")  
		else
			ril.request("AT+CGREG?")  
		end
	end
end

local function neturc(data, prefix)
    if prefix=="+COPS" then
        cops(data)
    elseif prefix == "+CREG" or prefix == "+CGREG" or prefix == "+CEREG" then
        csqQueryPoll()
        creg(data)
    elseif prefix == "+EEMLTESVC" or prefix == "+EEMLTEINTRA" or prefix == "+EEMLTEINTER" or prefix=="+EEMLTEINTERRAT" then
        eemLteSvc(data)
    elseif prefix == "+EEMUMTSSVC" then
        eemUMTSInfoSvc(data)
    elseif prefix == "+EEMGINFOSVC" then
        eemGsmInfoSvc(data)
    elseif prefix == "+EEMGINFONC" then
        eemGsmNCInfoSvc(data)   
    elseif prefix == "^MODE" then
        UpdNetMode(data)
    end
end

function switchFly(mode)
	if flyMode == mode then return end
	flyMode = mode
	if mode then
		ril.request("AT+CFUN=0")
	else
		ril.request("AT+CFUN=1")
		csqQueryPoll()
		cengQueryPoll()
		neturc("2", "+CREG")
	end
end

function getNetMode()
	return netMode
end

function getState()
	return state
end

function getMcc()
	return cellinfo[1].mcc and string.format("%x",cellinfo[1].mcc) or sim.getMcc()
end

function getMnc()
	return cellinfo[1].mnc and string.format("%x",cellinfo[1].mnc) or sim.getMnc()
end

function getLac()
	return lac
end

function getBand()
	return band
end

function getCi()
	return ci
end

function getRssi()
	return rssi
end

function getRsrp()
	return rsrp
end

function getCell()
	local i,ret = 1,""
	for i=1,cellinfo.cnt do
		if cellinfo[i] and cellinfo[i].lac and cellinfo[i].lac ~= 0 and cellinfo[i].ci and cellinfo[i].ci ~= 0 then
			ret = ret..cellinfo[i].ci.."."..cellinfo[i].rssi.."."
		end
	end
	return ret
end

function getCellInfo()
	local i, ret = 1, ""
	for i = 1, cellinfo.cnt do
		if cellinfo[i] and cellinfo[i].lac and cellinfo[i].lac ~= 0 and cellinfo[i].ci and cellinfo[i].ci ~= 0 then
			ret = ret .. cellinfo[i].lac .. "." .. cellinfo[i].ci .. "." .. cellinfo[i].rssi .. ";"
		end
	end
	return ret
end

function getCellInfoExt(rssi)
	local i, ret = 1, ""
	for i = 1, cellinfo.cnt do
		if cellinfo[i] and cellinfo[i].mcc and cellinfo[i].mnc and cellinfo[i].lac and cellinfo[i].lac ~= 0 and cellinfo[i].ci and cellinfo[i].ci ~= 0 then
			ret = ret .. string.format("%x",cellinfo[i].mcc) .. "." .. string.format("%x",cellinfo[i].mnc) .. "." .. cellinfo[i].lac .. "." .. cellinfo[i].ci .. "." .. (rssi and (cellinfo[i].rssi*2-113) or cellinfo[i].rssi) .. ";"
		end
	end
	return ret
end

function getTa()
	return cellinfo[1].ta
end

local function rsp(cmd, success, response, intermediate)
	local prefix = string.match(cmd, "AT(%+%u+)")
	
	if intermediate ~= nil then
		if prefix == "+CSQ" then
			local s = string.match(intermediate, "+CSQ:%s*(%d+)")
			if s ~= nil then
				rssi = tonumber(s)
				rssi = rssi == 99 and 0 or rssi
				publish("GSM_SIGNAL_REPORT_IND", success, rssi)
			end
		elseif prefix == "+CESQ" then
	        local s = string.match(intermediate, "+CESQ: %d+,%d+,%d+,%d+,%d+,(%d+)")
			if s ~= nil then
				rsrp = tonumber(s)
			end
		elseif prefix == "+CENG" then end
	end
    if prefix == "+CFUN" then
        if success then publish("FLYMODE", flyMode) end
    end
end

function getMultiCell(cbFnc)
	multicellcb = cbFnc

	ril.request("AT+EEMGINFO?")
end

function cengQueryPoll(period)

	if not flyMode then

		ril.request("AT+EEMGINFO?")
	else
		log.warn("net.cengQueryPoll", "flymode:", flyMode)
	end
	if nil ~= period then

		sys.timerStopAll(cengQueryPoll)
		sys.timerStart(cengQueryPoll, period, period)
	end
	return not flyMode
end

function csqQueryPoll(period)
    if not flyMode then        
        ril.request("AT+CSQ")
        ril.request("AT+CESQ")
    else
        log.warn("net.csqQueryPoll", "flymode:", flyMode)
    end
    if nil ~= period then
        sys.timerStopAll(csqQueryPoll)
        sys.timerStart(csqQueryPoll, period, period)
    end
    return not flyMode
end

function startQueryAll(...)
	local arg = { ... }
    csqQueryPoll(arg[1])
    cengQueryPoll(arg[2])
    if flyMode then        
        log.info("sim.startQuerAll", "flyMode:", flyMode)
    end
    return true
end

function stopQueryAll()
    sys.timerStopAll(csqQueryPoll)
    sys.timerStopAll(cengQueryPoll)
end

local sEngMode

function setEngMode(mode)
    sEngMode = mode or 1
    ril.request("AT+EEMOPT="..sEngMode,nil,function(cmd,success)
            function retrySetEngMode()
                setEngMode(sEngMode)
            end
            if success then
                sys.timerStop(retrySetEngMode)
            else
                sys.timerStart(retrySetEngMode,3000)
            end
        end)
end

sys.subscribe("SIM_IND", function(para)
	log.info("SIM.subscribe", simerrsta, para)
	if simerrsta ~= (para ~= "RDY") then
		simerrsta = (para ~= "RDY")
	end
	if para ~= "RDY" then
		state = "UNREGISTER"
		publish("NET_STATE_UNREGISTER")
	else
	end
end)

ril.regUrc("+COPS", neturc)
ril.regUrc("+CREG", neturc)
ril.regUrc("+CGREG", neturc)
ril.regUrc("+CEREG", neturc)
ril.regUrc("+EEMLTESVC", neturc)
ril.regUrc("+EEMLTEINTER", neturc)
ril.regUrc("+EEMLTEINTRA", neturc)
ril.regUrc("+EEMLTEINTERRAT", neturc)
ril.regUrc("+EEMGINFOSVC", neturc)
ril.regUrc("+EEMGINFONC", neturc)
ril.regUrc("+EEMUMTSSVC", neturc)
ril.regUrc("^MODE", neturc)
ril.regRsp("+CSQ", rsp)
ril.regRsp("+CESQ",rsp)
ril.regRsp("+CFUN", rsp)
ril.request("AT+COPS?")
ril.request("AT+CREG=2")
ril.request("AT+CGREG=2")
ril.request("AT+CEREG=2")
ril.request("AT+CREG?")
ril.request("AT+CGREG?")
ril.request("AT+CEREG?")
ril.request("AT+CALIBINFO?")
ril.request("AT*BAND?")
setEngMode(1)
resetCellInfo()
