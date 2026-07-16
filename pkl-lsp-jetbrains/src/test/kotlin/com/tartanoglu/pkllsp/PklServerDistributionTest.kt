package com.tartanoglu.pkllsp

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class PklServerDistributionTest {
    @Test
    fun detectsSupportedTargets() {
        assertEquals(PklServerDistribution.Target.LINUX_X64, PklServerDistribution.detectTarget("Linux", "x86_64"))
        assertEquals(PklServerDistribution.Target.LINUX_ARM64, PklServerDistribution.detectTarget("Linux", "aarch64"))
        assertEquals(PklServerDistribution.Target.WINDOWS_X64, PklServerDistribution.detectTarget("Windows 11", "amd64"))
        assertEquals(PklServerDistribution.Target.DARWIN_X64, PklServerDistribution.detectTarget("Mac OS X", "x86_64"))
        assertEquals(PklServerDistribution.Target.DARWIN_ARM64, PklServerDistribution.detectTarget("Darwin", "arm64"))
    }

    @Test
    fun rejectsUnsupportedTargets() {
        assertNull(PklServerDistribution.detectTarget("Linux", "riscv64"))
        assertNull(PklServerDistribution.detectTarget("Windows", "arm64"))
    }
}
