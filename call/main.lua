PROJECT = "CALL for LHC-monitoring-control-system"
VERSION = "1.0.0"
require "log"
LOG_LEVEL = log.LOGLEVEL_TRACE
require "sys"
require "netLed"
pmd.ldoset(2,pmd.LDO_VLCD)
netLed.setup(true,pio.P0_1,pio.P0_4)
require "testCall"
require "key"
sys.init(0, 0)
sys.run()
