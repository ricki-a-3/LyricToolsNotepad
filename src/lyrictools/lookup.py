"""Background word lookups.

Every lookup -- even the offline rhyme modes -- runs off the UI thread, and
results come back through a Qt signal. Offline and network work use separate
single-thread executors, so a slow Datamuse call (Synonyms / Deep Search) can
never hold up the offline dropdown, and the offline engine's lazily built
indexes are only ever touched by one worker at a time.
"""
from concurrent.futures import ThreadPoolExecutor

from PySide6.QtCore import QObject, Signal

from . import core

DROPDOWN_RESULTS_PER_CATEGORY = 60
DEEP_SEARCH_MAX_RESULTS = 150
NETWORK_TIMEOUT = 6

MODE_PERFECT = "rhyme"
MODE_NEAR = "near"
MODE_REPORT = "report"
MODE_SYNONYM = "synonym"
MODE_DEEP = "deep"

ARMED_MODES = (MODE_PERFECT, MODE_NEAR, MODE_REPORT)
NETWORK_MODES = (MODE_SYNONYM, MODE_DEEP)

MODE_TITLES = {
    MODE_PERFECT: "Perfect Rhymes",
    MODE_NEAR: "Near / Slant Rhymes",
    MODE_REPORT: "Perfect + Near Rhymes",
    MODE_SYNONYM: "Synonyms",
    MODE_DEEP: "Deep Search",
}


def collect(word, mode):
    """Run one lookup synchronously. Returns a list of (heading, results)
    groups; each result is core's {"word", "syllables", "score"} dict."""
    if mode == MODE_PERFECT:
        return [("Perfect", core.find_words_local(word, "rhyme", DROPDOWN_RESULTS_PER_CATEGORY))]
    if mode == MODE_NEAR:
        return [("Near", core.find_words_local(word, "near", DROPDOWN_RESULTS_PER_CATEGORY))]
    if mode == MODE_REPORT:
        perfect = core.find_words_local(word, "rhyme", DROPDOWN_RESULTS_PER_CATEGORY)
        near = core.find_words_local(word, "near", DROPDOWN_RESULTS_PER_CATEGORY)
        return [("Perfect", perfect), ("Near", near)]
    if mode == MODE_SYNONYM:
        results = core.find_words(word, "synonym", DROPDOWN_RESULTS_PER_CATEGORY, timeout=NETWORK_TIMEOUT)
        return [("Synonyms", results)]
    if mode == MODE_DEEP:
        results = core.find_words_deep(word, DEEP_SEARCH_MAX_RESULTS, timeout=NETWORK_TIMEOUT)
        return [("Deep Search", results)]
    raise ValueError("Unknown mode: %s" % mode)


def warm_up():
    """Load CMUdict and build the rhyme indexes so the first highlight is fast."""
    core.load_cmudict()
    core.load_cmu_pronunciations()
    core._build_rhyme_indexes()


class LookupService(QObject):
    """Runs lookups on worker threads; only the newest request is delivered."""

    # request_id, word, mode, groups (list) or None, error message or ""
    finished = Signal(int, str, str, object, str)

    def __init__(self, parent=None):
        super().__init__(parent)
        self._offline = ThreadPoolExecutor(max_workers=1, thread_name_prefix="lyric-offline")
        self._network = ThreadPoolExecutor(max_workers=1, thread_name_prefix="lyric-network")
        self._latest = 0
        self._offline.submit(warm_up)

    def request(self, word, mode):
        """Start a lookup; returns its id. Any older pending request is
        superseded and its result will be dropped."""
        self._latest += 1
        request_id = self._latest
        pool = self._network if mode in NETWORK_MODES else self._offline
        future = pool.submit(collect, word, mode)
        future.add_done_callback(lambda f: self._done(request_id, word, mode, f))
        return request_id

    def cancel(self):
        """Drop whatever is in flight (the work itself finishes harmlessly)."""
        self._latest += 1

    def is_current(self, request_id):
        return request_id == self._latest

    def _done(self, request_id, word, mode, future):
        # Runs on the worker thread; the signal is queued to the UI thread.
        try:
            groups, error = future.result(), ""
        except Exception as exc:  # network down, timeout, bad payload...
            groups, error = None, str(exc) or exc.__class__.__name__
        self.finished.emit(request_id, word, mode, groups, error)

    def shutdown(self):
        self._offline.shutdown(wait=False, cancel_futures=True)
        self._network.shutdown(wait=False, cancel_futures=True)
