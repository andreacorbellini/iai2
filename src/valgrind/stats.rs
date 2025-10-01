#[derive(Clone, Debug)]
pub(crate) struct CachegrindStats {
    pub(crate) instruction_reads: u64,
    pub(crate) instruction_l1_misses: u64,
    pub(crate) instruction_cache_misses: u64,
    pub(crate) data_reads: u64,
    pub(crate) data_l1_read_misses: u64,
    pub(crate) data_cache_read_misses: u64,
    pub(crate) data_writes: u64,
    pub(crate) data_l1_write_misses: u64,
    pub(crate) data_cache_write_misses: u64,
}

impl CachegrindStats {
    pub(crate) fn ram_accesses(&self) -> u64 {
        self.instruction_cache_misses + self.data_cache_read_misses + self.data_cache_write_misses
    }

    pub(crate) fn summarize(&self) -> CachegrindSummary {
        let ram_hits = self.ram_accesses();
        let l3_accesses =
            self.instruction_l1_misses + self.data_l1_read_misses + self.data_l1_write_misses;
        let l3_hits = l3_accesses - ram_hits;

        let total_memory_rw = self.instruction_reads + self.data_reads + self.data_writes;
        let l1_hits = total_memory_rw - (ram_hits + l3_hits);

        CachegrindSummary {
            l1_hits,
            l3_hits,
            ram_hits,
        }
    }

    pub(crate) fn subtract(&self, calibration: &CachegrindStats) -> CachegrindStats {
        CachegrindStats {
            instruction_reads: self
                .instruction_reads
                .saturating_sub(calibration.instruction_reads),
            instruction_l1_misses: self
                .instruction_l1_misses
                .saturating_sub(calibration.instruction_l1_misses),
            instruction_cache_misses: self
                .instruction_cache_misses
                .saturating_sub(calibration.instruction_cache_misses),
            data_reads: self.data_reads.saturating_sub(calibration.data_reads),
            data_l1_read_misses: self
                .data_l1_read_misses
                .saturating_sub(calibration.data_l1_read_misses),
            data_cache_read_misses: self
                .data_cache_read_misses
                .saturating_sub(calibration.data_cache_read_misses),
            data_writes: self.data_writes.saturating_sub(calibration.data_writes),
            data_l1_write_misses: self
                .data_l1_write_misses
                .saturating_sub(calibration.data_l1_write_misses),
            data_cache_write_misses: self
                .data_cache_write_misses
                .saturating_sub(calibration.data_cache_write_misses),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CachegrindSummary {
    pub(crate) l1_hits: u64,
    pub(crate) l3_hits: u64,
    pub(crate) ram_hits: u64,
}

impl CachegrindSummary {
    pub(crate) fn cycles(&self) -> u64 {
        // Uses Itamar Turner-Trauring's formula from https://pythonspeed.com/articles/consistent-benchmarking-in-ci/
        self.l1_hits + (5 * self.l3_hits) + (35 * self.ram_hits)
    }
}
