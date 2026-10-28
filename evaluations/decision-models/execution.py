"""Bounded, ordered evaluation calls with cooperative cancellation."""

from collections import deque
from concurrent.futures import ThreadPoolExecutor, TimeoutError, wait
from threading import Event


CANCEL_GRACE_SECONDS = 5.0


def bounded_map(function, items, workers=1, should_stop=None, cancel_event=None, skipped=None):
    """Yield (item, result) in input order, with at most workers submitted calls.

    A soft halt drains active calls and yields placeholders for everything unsent.
    Cancellation raises KeyboardInterrupt after a bounded drain. It cannot kill
    running I/O: the function must check the shared event before paid requests,
    and its transport timeout and persistent ledger still apply to active calls.
    """
    if isinstance(workers, bool) or not isinstance(workers, int) or not 1 <= workers <= 8:
        raise ValueError("evaluation workers must be an integer from 1 to 8")
    cancelled = cancel_event if cancel_event is not None else Event()
    placeholder = skipped or (lambda item: {
        "status": "unavailable", "prediction": None, "reason": "execution_halted_before_submission",
    })
    source = iter(items)
    pending = deque()
    stopped = exhausted = completed = False
    executor = ThreadPoolExecutor(max_workers=workers, thread_name_prefix="pennel-evaluation")

    def check_cancelled():
        if cancelled.is_set():
            raise KeyboardInterrupt("Evaluation cancelled")

    def check_stopped():
        nonlocal stopped
        if should_stop is not None and should_stop():
            stopped = True
        if stopped:
            for _, future in pending:
                future.cancel()
        return stopped

    def fill():
        nonlocal exhausted
        while len(pending) < workers and not exhausted:
            check_cancelled()
            if check_stopped():
                return
            try:
                item = next(source)
            except StopIteration:
                exhausted = True
                return
            pending.append((item, executor.submit(function, item)))

    try:
        fill()
        while pending:
            check_cancelled()
            check_stopped()
            item, future = pending[0]
            while True:
                check_cancelled()
                if future.cancelled():
                    result = placeholder(item)
                    break
                try:
                    result = future.result(timeout=.05)
                    break
                except TimeoutError:
                    if future.done():
                        result = future.result()
                        break
                    check_stopped()
            pending.popleft()
            yield item, result
            fill()
        if stopped:
            for item in source:
                check_cancelled()
                yield item, placeholder(item)
        completed = True
    finally:
        if not completed:
            cancelled.set()
            for _, future in pending:
                future.cancel()
            try:
                wait([future for _, future in pending], timeout=CANCEL_GRACE_SECONDS)
            finally:
                executor.shutdown(wait=False, cancel_futures=True)
        else:
            executor.shutdown(wait=True)
