const fs = require('fs');
const path = require('path');

/**
 * 从共享配置文件生成前端环境文件
 */
function generateFrontendEnvironments() {
  const configPath = path.join(__dirname, '..', '..', 'conf', 'shared.json');
  const envDevPath = path.join(__dirname, '..', '..', 'frontend', 'src', 'environments', 'environment.ts');
  const envProdPath = path.join(__dirname, '..', '..', 'frontend', 'src', 'environments', 'environment.prod.ts');

  if (!fs.existsSync(configPath)) {
    console.error(`❌ 配置文件不存在: ${configPath}`);
    process.exit(1);
  }

  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));

  // 生成开发环境文件
  const devConfig = config.dev;
  const devContent = `// Development environment configuration
// This file is auto-generated from conf/shared.json
// To regenerate: npm run config:generate

export const environment = {
  production: false,
  apiUrl: '${devConfig.frontend.apiUrl}',
  backendPort: ${devConfig.backend.port},
  frontendPort: ${devConfig.frontend.port},
};
`;

  // 生成生产环境文件
  const prodConfig = config.prod;
  const prodContent = `// Production environment configuration
// This file is auto-generated from conf/shared.json
// To regenerate: npm run config:generate

export const environment = {
  production: true,
  apiUrl: '${prodConfig.frontend.apiUrl}',
  backendPort: ${prodConfig.backend.port},
  frontendPort: ${prodConfig.frontend.port},
};
`;

  fs.writeFileSync(envDevPath, devContent);
  fs.writeFileSync(envProdPath, prodContent);

  console.log('✅ 前端环境文件已生成:');
  console.log(`   - ${envDevPath}`);
  console.log(`   - ${envProdPath}`);
}

// 如果直接运行此脚本
if (require.main === module) {
  generateFrontendEnvironments();
}

module.exports = { generateFrontendEnvironments };

