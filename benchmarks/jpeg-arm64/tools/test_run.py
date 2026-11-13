import csv
import io
import unittest

from profile_gdb import region
from run import pair_values, parse_expected, parse_measurements, validate_ref


class MeasurementTests(unittest.TestCase):
    def setUp(self):
        self.expected = {"one": {"primary": 1, "iterations": 2, "rows": 3},
                         "two": {"primary": 0, "iterations": 2, "rows": 4}}

    def samples(self, updates=None):
        rows = [
            {"case": "one", "primary": 1, "ns_per_image": 60000, "batch_ns": 120000,
             "iterations": 2, "calls": 1, "suspensions": 0, "rows": 3},
            {"case": "two", "primary": 0, "ns_per_image": 70000, "batch_ns": 140000,
             "iterations": 2, "calls": 2, "suspensions": 1, "rows": 4},
        ]
        if updates:
            rows[0].update(updates)
        output = io.StringIO()
        writer = csv.DictWriter(output, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
        return output.getvalue()

    def test_empty_missing_and_duplicate_cases_fail(self):
        for text in ("", self.samples().splitlines()[0] + "\n",
                     self.samples() + self.samples().splitlines()[1] + "\n"):
            with self.assertRaises(ValueError):
                parse_measurements(text, self.expected)
        with self.assertRaises(ValueError):
            parse_measurements("", {})
        with self.assertRaises(ValueError):
            parse_expected("case,primary,iterations,rows\n")

    def test_invalid_or_unfinished_samples_fail(self):
        for updates in ({"ns_per_image": "NaN"}, {"rows": 0}, {"calls": 0},
                        {"primary": 0}, {"iterations": 1}, {"batch_ns": 0},
                        {"ns_per_image": 1, "batch_ns": 2}):
            with self.subTest(updates=updates), self.assertRaises(ValueError):
                parse_measurements(self.samples(updates), self.expected)
        self.assertEqual(parse_measurements(self.samples(), self.expected), {"one": 60000, "two": 70000})

    def test_primary_and_protected_gains_remain_paired(self):
        pairs = [({"one": 100, "two": 1000}, {"one": 90, "two": 2000}),
                 ({"one": 200, "two": 4000}, {"one": 180, "two": 2000})]
        values, times = pair_values(pairs, self.expected)
        self.assertAlmostEqual(values["PRIMARY"][0], 0.1)
        self.assertAlmostEqual(values["PRIMARY"][1], 0.1)
        self.assertEqual(values["two"], [-1.0, 0.5])
        self.assertAlmostEqual(times[0][0], 100)

    def test_profile_classifies_real_functions_not_caller_types(self):
        self.assertEqual(region("zune_jpeg::idct::neon::idct_neon"), "idct_neon")
        self.assertEqual(region("zune_jpeg::color_convert::neon64::ycbcr_to_rgb_neon"), "color_neon")
        self.assertEqual(region("std::drop_in_place<zune_jpeg::idct::neon::idct_neon>"),
                         "other_setup_or_unclassified")

    def test_ref_is_not_a_shell_or_git_option(self):
        for value in ("--upload-pack=evil", "dev;echo bad", "../dev", "dev\nbad", ""):
            with self.assertRaises(ValueError):
                validate_ref(value)
        self.assertEqual(validate_ref("dev"), "dev")
        self.assertEqual(validate_ref("a" * 40), "a" * 40)


if __name__ == "__main__":
    unittest.main()
