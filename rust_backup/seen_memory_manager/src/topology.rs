//! Memory topology preferences/hints shared across the memory manager.

/// Physical tier used when placing a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryTier {
    /// Default host-attached memory.
    Host,
    /// CXL memory positioned near compute (low latency).
    CxlNear,
    /// CXL memory positioned farther from compute (high capacity).
    CxlFar,
}

impl MemoryTier {
    pub fn label(self) -> &'static str {
        match self {
            MemoryTier::Host => "host",
            MemoryTier::CxlNear => "cxl-near",
            MemoryTier::CxlFar => "cxl-far",
        }
    }
}

/// User-facing preference for how the allocator should place regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryTopologyPreference {
    /// Use host memory everywhere (status quo).
    Default,
    /// Keep hot regions near compute and spill cold/large regions to far CXL.
    CxlNear,
    /// Force all regions into far CXL (maximum capacity testing).
    CxlFar,
}

impl Default for MemoryTopologyPreference {
    fn default() -> Self {
        MemoryTopologyPreference::Default
    }
}
