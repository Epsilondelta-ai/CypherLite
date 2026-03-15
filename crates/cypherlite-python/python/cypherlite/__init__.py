"""CypherLite: An embedded graph database for Python."""

from importlib.metadata import version as _pkg_version

from cypherlite._cypherlite import (
    CypherLiteError,
    Database,
    EdgeID,
    NodeID,
    Result,
    Transaction,
    features,
    open,
    version,
)

__version__ = _pkg_version("cypherlite")

__all__ = [
    "__version__",
    "open",
    "version",
    "features",
    "Database",
    "Result",
    "Transaction",
    "CypherLiteError",
    "NodeID",
    "EdgeID",
]
