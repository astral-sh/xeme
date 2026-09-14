from typing import Literal, TypeAlias

__version__: str

EventKind: TypeAlias = Literal[
    "start",
    "end",
    "text",
    "comment",
    "pi",
    "start_ns",
    "end_ns",
    "start_cdata",
    "end_cdata",
    "xml_decl",
    "start_doctype",
    "end_doctype",
]
EventData: TypeAlias = (
    str
    | tuple[str, dict[str, str]]
    | tuple[str, str]
    | tuple[str | None, str | None]
    | tuple[str, str | None, bool | None]
    | tuple[str, str | None, str | None, bool]
    | None
)

class Position:
    @property
    def line(self) -> int: ...
    @property
    def column(self) -> int: ...
    @property
    def byte_index(self) -> int: ...
    @property
    def byte_count(self) -> int: ...

class Event:
    @property
    def kind(self) -> EventKind: ...
    @property
    def data(self) -> EventData: ...
    @property
    def position(self) -> Position: ...

class Limits:
    def __init__(
        self,
        *,
        max_depth: int = 256,
        max_token_bytes: int = 16_777_216,
        max_total_bytes: int = 268_435_456,
        max_entity_expansion_bytes: int = 8_388_608,
        max_entity_depth: int = 32,
        max_attributes: int = 10_000,
        max_entities: int = 10_000,
    ) -> None: ...
    @property
    def max_depth(self) -> int: ...
    @property
    def max_token_bytes(self) -> int: ...
    @property
    def max_total_bytes(self) -> int: ...
    @property
    def max_entity_expansion_bytes(self) -> int: ...
    @property
    def max_entity_depth(self) -> int: ...
    @property
    def max_attributes(self) -> int: ...
    @property
    def max_entities(self) -> int: ...

class ParseError(Exception):
    kind: str
    line: int
    column: int
    byte_index: int

class Parser:
    def __init__(
        self,
        *,
        encoding: str | None = None,
        namespace_separator: str | None = None,
        namespace_triplets: bool = False,
        limits: Limits | None = None,
    ) -> None: ...
    def feed(self, data: bytes, final: bool = False) -> None: ...
    def next_event(self) -> Event | None: ...
