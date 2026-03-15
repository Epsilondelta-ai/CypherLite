// Auto-generated loader for the CypherLite native addon.
// This file loads the platform-specific .node binary.

const { existsSync } = require('node:fs');
const { join } = require('node:path');

// Try to load the .node binary from the current directory.
const localPath = join(__dirname, 'cypherlite.node');

let nativeBinding;

if (existsSync(localPath)) {
  nativeBinding = require(localPath);
} else {
  throw new Error(
    `Failed to load cypherlite native module. Expected at: ${localPath}`
  );
}

module.exports = nativeBinding;
