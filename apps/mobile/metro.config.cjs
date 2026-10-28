const { getDefaultConfig } = require('expo/metro-config');
const path = require('node:path');

const config = getDefaultConfig(__dirname);
config.watchFolders = [path.resolve(__dirname, '../../assets')];
config.resolver.nodeModulesPaths = [path.join(__dirname, 'node_modules'), ...(config.resolver.nodeModulesPaths ?? [])];
module.exports = config;
