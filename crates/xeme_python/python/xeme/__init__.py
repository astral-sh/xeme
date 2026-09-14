"""Incremental XML parsing with owned events and configurable resource limits."""

from collections.abc import Iterator

from ._native import Event, Limits, ParseError, Position, __version__
from ._native import Parser as _Parser

__all__ = ["Event", "Limits", "ParseError", "Parser", "Position", "__version__"]


class Parser:
    """Parse XML bytes incrementally, draining events after every input chunk.

    Namespace processing is enabled by a one-character ``namespace_separator``.
    ``encoding`` requests an input encoding. ``limits`` defaults to ``Limits()``.
    """

    __slots__ = ("_parser",)

    def __init__(
        self,
        *,
        encoding: str | None = None,
        namespace_separator: str | None = None,
        namespace_triplets: bool = False,
        limits: Limits | None = None,
    ) -> None:
        self._parser = _Parser(
            encoding=encoding,
            namespace_separator=namespace_separator,
            namespace_triplets=namespace_triplets,
            limits=limits,
        )

    def feed(self, data: bytes, final: bool = False) -> None:
        """Supply bytes; set ``final=True`` on the last chunk, which may be empty.

        Exhaust ``read_events()`` before feeding again. Parse errors are terminal
        and can be raised here or while reading events.
        """
        self._parser.feed(data, final=final)

    def read_events(self) -> Iterator[Event]:
        """Yield available events until the parser needs input or has finished.

        Events own their data and can be retained after subsequent input. Text
        can span multiple events; event boundaries can depend on input chunks.
        """
        while (event := self._parser.next_event()) is not None:
            yield event
