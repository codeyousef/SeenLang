"""GPU tool admission must distinguish reclaimable build cache from live memory."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from gpu_toolchain_process import cgroup_headroom, ToolProcessError


class ToolchainMemoryContract(unittest.TestCase):
    def test_cached_build_can_admit_a_bounded_validator(self):
        gib = 1024**3
        headroom = cgroup_headroom(5 * gib, 9 * gib // 2, {
            'inactive_file': gib, 'file_dirty': gib // 8, 'file_writeback': gib // 8})
        self.assertEqual(headroom, 5 * gib // 4)
        self.assertGreaterEqual(headroom // 4, 256 * 1024**2)

    def test_missing_statistics_keep_the_strict_remaining_budget(self):
        self.assertEqual(cgroup_headroom(500, 420, {}), 80)

    def test_dirty_and_writeback_cache_is_not_admitted(self):
        self.assertEqual(cgroup_headroom(500, 420, {
            'inactive_file': 100, 'file_dirty': 70, 'file_writeback': 50}), 80)

    def test_active_file_count_alone_cannot_establish_clean_cache(self):
        self.assertEqual(cgroup_headroom(500, 420, {'active_file': 300}), 80)

    def test_clean_unmapped_recent_build_cache_is_reclaimable(self):
        self.assertEqual(cgroup_headroom(500, 420, {
            'file': 300, 'active_file': 250, 'inactive_file': 50}), 380)

    def test_mapped_and_shared_memory_stays_in_the_working_set(self):
        self.assertEqual(cgroup_headroom(500, 420, {
            'file': 300, 'file_mapped': 70, 'shmem': 50,
            'file_dirty': 20, 'file_writeback': 10}), 230)

    def test_present_file_total_wins_over_a_stale_inactive_counter(self):
        self.assertEqual(cgroup_headroom(500, 420, {'file': 100, 'inactive_file': 300}), 180)

    def test_transient_statistic_skew_cannot_exceed_the_cap(self):
        self.assertEqual(cgroup_headroom(500, 420, {'inactive_file': 1000}), 500)

    def test_over_limit_live_memory_has_no_headroom(self):
        self.assertEqual(cgroup_headroom(500, 600, {}), 0)

    def test_negative_memory_accounting_is_rejected(self):
        for maximum, current, statistics in [(-1, 0, {}), (500, -1, {}),
                (500, 420, {'inactive_file': -1}),
                (500, 420, {'file_dirty': -1}), (500, 420, {'file_writeback': -1}),
                (500, 420, {'file': -1}), (500, 420, {'file_mapped': -1}),
                (500, 420, {'shmem': -1})]:
            with self.subTest(maximum=maximum, current=current, statistics=statistics):
                with self.assertRaises(ToolProcessError):
                    cgroup_headroom(maximum, current, statistics)


if __name__ == '__main__':
    unittest.main()
