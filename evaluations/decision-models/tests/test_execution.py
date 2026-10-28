import importlib.util
from pathlib import Path
import threading
import time
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location("decision_execution", Path(__file__).resolve().parents[1] / "execution.py")
execution = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(execution)


class BoundedExecutionTests(unittest.TestCase):
    def test_results_stay_ordered_and_concurrency_is_bounded(self):
        lock = threading.Lock()
        active = peak = 0
        def work(item):
            nonlocal active, peak
            with lock:
                active += 1
                peak = max(peak, active)
            time.sleep((3 - item % 3) * .001)
            with lock:
                active -= 1
            return item * 2
        result = list(execution.bounded_map(work, range(30), workers=3))
        self.assertEqual(result, [(item, item * 2) for item in range(30)])
        self.assertLessEqual(peak, 3)
        self.assertGreater(peak, 1)

    def test_does_not_eagerly_consume_a_thousand_items(self):
        consumed = []
        started = threading.Event()
        release = threading.Event()
        starts = []
        lock = threading.Lock()
        def source():
            for item in range(1000):
                consumed.append(item)
                yield item
        def work(item):
            with lock:
                starts.append(item)
                if len(starts) == 3:
                    started.set()
            release.wait(2)
            return item
        iterator = execution.bounded_map(work, source(), workers=3)
        first = []
        thread = threading.Thread(target=lambda: first.append(next(iterator)))
        thread.start()
        try:
            self.assertTrue(started.wait(1))
            self.assertEqual(consumed, [0, 1, 2])
        finally:
            release.set()
            thread.join(2)
            iterator.close()
        self.assertEqual(first, [(0, 0)])
        self.assertEqual(consumed, [0, 1, 2])

    def test_halt_keeps_unsent_items_as_placeholders(self):
        halted = threading.Event()
        starts = []
        def work(item):
            starts.append(item)
            if item == 0:
                halted.set()
            return {"status": "ok", "value": item}
        result = list(execution.bounded_map(work, range(100), workers=3,
                                           should_stop=halted.is_set,
                                           skipped=lambda item: {"status": "unavailable", "value": item}))
        self.assertEqual([item for item, _ in result], list(range(100)))
        self.assertLessEqual(len(starts), 3)
        self.assertTrue(all(value["status"] == "unavailable" for _, value in result[3:]))
        self.assertEqual(sum(value["status"] == "ok" for _, value in result), len(starts))

    def test_initial_halt_never_starts_a_call(self):
        starts = []
        result = list(execution.bounded_map(lambda item: starts.append(item), range(5), should_stop=lambda: True))
        self.assertFalse(starts)
        self.assertEqual(len(result), 5)
        self.assertTrue(all(value["status"] == "unavailable" for _, value in result))

    def test_cancellation_starts_no_further_calls(self):
        cancel = threading.Event()
        started = threading.Event()
        release = threading.Event()
        starts = []
        errors = []
        lock = threading.Lock()
        def work(item):
            with lock:
                starts.append(item)
                if len(starts) == 2:
                    started.set()
            release.wait(2)
            return item
        def consume():
            try:
                list(execution.bounded_map(work, range(100), workers=2, cancel_event=cancel))
            except BaseException as error:
                errors.append(error)
        thread = threading.Thread(target=consume)
        thread.start()
        try:
            self.assertTrue(started.wait(1))
            cancel.set()
        finally:
            release.set()
            thread.join(2)
        self.assertFalse(thread.is_alive())
        self.assertEqual(len(starts), 2)
        self.assertEqual(len(errors), 1)
        self.assertIsInstance(errors[0], KeyboardInterrupt)

    def test_keyboard_interrupt_sets_shared_cancel_event(self):
        cancel = threading.Event()
        def work(item):
            raise KeyboardInterrupt()
        with self.assertRaises(KeyboardInterrupt):
            list(execution.bounded_map(work, range(10), workers=1, cancel_event=cancel))
        self.assertTrue(cancel.is_set())

    def test_one_worker_preserves_sequential_call_order(self):
        starts = []
        result = list(execution.bounded_map(lambda item: starts.append(item) or item, range(5)))
        self.assertEqual(starts, list(range(5)))
        self.assertEqual(result, [(item, item) for item in range(5)])

    def test_worker_bounds_are_explicit(self):
        for workers in (0, -1, 9, True, 1.5):
            with self.assertRaises(ValueError):
                list(execution.bounded_map(lambda item: item, [], workers=workers))

    def test_worker_timeout_error_is_not_treated_as_a_poll_timeout(self):
        starts = []
        def work(item):
            starts.append(item)
            raise TimeoutError("request timeout")
        with self.assertRaisesRegex(TimeoutError, "request timeout"):
            list(execution.bounded_map(work, range(10)))
        self.assertEqual(starts, [0])

    def test_cancellation_drain_is_bounded_for_uncooperative_call(self):
        cancelled = threading.Event()
        started = threading.Event()
        release = threading.Event()
        errors = []
        def work(item):
            started.set()
            release.wait(2)
            return item
        def consume():
            try:
                list(execution.bounded_map(work, range(10), cancel_event=cancelled))
            except BaseException as error:
                errors.append(error)
        with patch.object(execution, "CANCEL_GRACE_SECONDS", .02):
            thread = threading.Thread(target=consume)
            thread.start()
            try:
                self.assertTrue(started.wait(1))
                cancelled.set()
                thread.join(.5)
                self.assertFalse(thread.is_alive())
                self.assertIsInstance(errors[0], KeyboardInterrupt)
            finally:
                release.set()
                thread.join(2)


if __name__ == "__main__":
    unittest.main()
