"""CypherLite: An embedded graph database for Python."""

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

__version__ = version()

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
