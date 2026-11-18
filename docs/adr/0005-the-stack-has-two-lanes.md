# The stack has two lanes, split after the front pedals, blended with a power-law

The Guitar Amp's stack holds at most two lanes (`MAX_AMPS`). Four were tried and cost 7.8 to 9.0 % of a core at Gain 10, over the 8 % limit, so the limit is two (engineer's decision on #269). This does not limit how many amp models there are to choose from.

How the two lanes work (#177):

- The signal splits after the front pedals, so both lanes hear the same gate and drive, and the back pedals work on the mix. Each lane is a preamp, tone stack, power amp and cabinet in the same oversampled region, run on a copy of the signal.
- The mix sits behind the two cabinets and in front of the DC blocker, so both reach the back pedals in phase, with one oversampler and one cabinet latency. The chain order does not know about it: the stack is still one place in the order.
- Lane 1 is always in the signal. Lane 2 has a switch, and while it is off its stages are not run, the output is bit-identical to the single amp it was, and every project from before opens with it off.
- The blend is a power law: the first lane's share is `(1 - blend)^0.6`, the second's `blend^0.6`, exactly 1 and 0 at the ends. Linear dipped 2.8 LU in the middle for a heavy and a clean amp; equal power is flat for those but bulges 3 dB for two similar amps and pushes them over the safety clip's knee.

## Not done, on purpose

Phase invert on lane 2, stereo (lane 1 left, lane 2 right) and separate effect routing per lane (a split in front of the pedals). The first two only if a report or a listen shows a need; the third is a future issue.
