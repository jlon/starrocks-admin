const fs = require('fs');
const path = require('path');

/**
 * 读取共享配置文件并生成前端环境配置
 * @param {string} env - 环境名称 (dev/prod)
 */
function generateFrontendConfig(env = 'dev') {
  const configPath = path.join(__dirname, '..', 'conf', 'shared.json');
  
  if (!fs.existsSync(configPath)) {
    console.error(`❌ 配置文件不存在: ${configPath}`);
    process.exit(1);
  }

  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  const envConfig = config[env];

  if (!envConfig) {
    console.error(`❌ 环境配置不存在: ${env}`);
    process.exit(1);
  }

  const frontendConfig = {
    production: env === 'prod',
    apiUrl: envConfig.frontend.apiUrl,
    backendPort: envConfig.backend.port,
    frontendPort: envConfig.frontend.port,
  };

  return frontendConfig;
}

// 如果直接运行此脚本
if (require.main === module) {
  const env = process.argv[2] || 'dev';
  const config = generateFrontendConfig(env);
  console.log(JSON.stringify(config, null, 2));
}

module.exports = { generateFrontendConfig };

