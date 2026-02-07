#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

const version = process.argv[2];

if (!version) {
  console.error('Usage: node update-version.js <version>');
  process.exit(1);
}

console.log(`Updating version to ${version}`);

// Update main package.json
const mainPkgPath = path.join(__dirname, '..', 'package.json');
const mainPkg = JSON.parse(fs.readFileSync(mainPkgPath, 'utf8'));
mainPkg.version = version;

// Update optionalDependencies versions
for (const dep of Object.keys(mainPkg.optionalDependencies || {})) {
  mainPkg.optionalDependencies[dep] = version;
}
fs.writeFileSync(mainPkgPath, JSON.stringify(mainPkg, null, 2) + '\n');
console.log(`Updated ${mainPkgPath}`);

// Update platform package.json files
const platforms = ['darwin-arm64', 'darwin-x64', 'linux-x64', 'linux-arm64'];
for (const platform of platforms) {
  const pkgPath = path.join(__dirname, '..', 'npm', platform, 'package.json');
  if (fs.existsSync(pkgPath)) {
    const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
    pkg.version = version;
    fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
    console.log(`Updated ${pkgPath}`);
  }
}

// Update Cargo.toml
const cargoPath = path.join(__dirname, '..', 'Cargo.toml');
if (fs.existsSync(cargoPath)) {
  let cargo = fs.readFileSync(cargoPath, 'utf8');
  cargo = cargo.replace(/^version\s*=\s*"[^"]*"/m, `version = "${version}"`);
  fs.writeFileSync(cargoPath, cargo);
  console.log(`Updated ${cargoPath}`);
}

// Update package-lock.json directly (can't use npm install because new packages don't exist on npm yet)
const lockPath = path.join(__dirname, '..', 'package-lock.json');
if (fs.existsSync(lockPath)) {
  let lockContent = fs.readFileSync(lockPath, 'utf8');
  const lockJson = JSON.parse(lockContent);

  // Update the root version
  lockJson.version = version;

  // Update optionalDependencies in packages[""] (root)
  if (lockJson.packages && lockJson.packages['']) {
    const root = lockJson.packages[''];
    for (const dep of Object.keys(root.optionalDependencies || {})) {
      root.optionalDependencies[dep] = version;
    }
  }

  // Update each platform package entry in packages
  const platformPkgs = [
    '@hotwired-sh/cli-darwin-arm64',
    '@hotwired-sh/cli-darwin-x64',
    '@hotwired-sh/cli-linux-x64',
    '@hotwired-sh/cli-linux-arm64'
  ];
  for (const pkg of platformPkgs) {
    const key = `node_modules/${pkg}`;
    if (lockJson.packages && lockJson.packages[key]) {
      lockJson.packages[key].version = version;
    }
  }

  fs.writeFileSync(lockPath, JSON.stringify(lockJson, null, 2) + '\n');
  console.log(`Updated ${lockPath}`);
}

console.log('Version update complete');
