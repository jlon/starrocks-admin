import { Injectable } from '@angular/core';

export interface DiceBearOptions {
  seed?: string;
  backgroundColor?: string[];
  hair?: string[];
  hairColor?: string[];
  skinColor?: string[];
  eyes?: string[];
  eyebrows?: string[];
  mouth?: string[];
  accessories?: string[];
  accessoriesProbability?: number;
  clothing?: string[];
  clothingColor?: string[];
  flip?: boolean;
}

@Injectable({
  providedIn: 'root'
})
export class DiceBearService {
  private readonly baseUrl = 'https://api.dicebear.com/7.x';
  
  // 可用的头像样式 - 积极向上、多样化的风格
  readonly avatarStyles = [
    { name: 'avataaars', label: '经典人物', description: '经典卡通人物风格' },
    { name: 'avataaars-neutral', label: '简约人物', description: '简约中性人物风格' },
    { name: 'adventurer', label: '冒险家', description: '可爱卡通冒险家' },
    { name: 'adventurer-neutral', label: '简约冒险家', description: '简约冒险家风格' },
    { name: 'big-ears', label: '大耳朵', description: '可爱大耳朵风格' },
    { name: 'big-ears-neutral', label: '简约大耳朵', description: '简约大耳朵风格' },
    { name: 'big-smile', label: '大笑脸', description: '开心大笑脸风格' },
    { name: 'bottts', label: '机器人', description: '可爱机器人风格' },
    { name: 'croodles', label: '涂鸦', description: '手绘涂鸦风格' },
    { name: 'croodles-neutral', label: '简约涂鸦', description: '简约涂鸦风格' },
    { name: 'fun-emoji', label: '趣味表情', description: '有趣的表情符号' },
    { name: 'lorelei', label: '洛蕾莱', description: '现代简约风格' },
    { name: 'lorelei-neutral', label: '简约洛蕾莱', description: '极简洛蕾莱风格' },
    { name: 'micah', label: '米卡', description: '温暖插画风格' },
    { name: 'miniavs', label: '迷你头像', description: '迷你可爱风格' },
    { name: 'notionists', label: 'Notion风格', description: '专业Notion风格' },
    { name: 'notionists-neutral', label: '简约Notion', description: '简约Notion风格' },
    { name: 'open-peeps', label: '开放人物', description: '友好开放人物' },
    { name: 'personas', label: '人物角色', description: '多彩人物角色' },
    { name: 'pixel-art', label: '像素艺术', description: '复古像素风格' },
    { name: 'pixel-art-neutral', label: '简约像素', description: '简约像素风格' },
    { name: 'thumbs', label: '点赞', description: '积极点赞风格' },
    { name: 'shapes', label: '几何图形', description: '抽象几何风格' },
    { name: 'icons', label: '图标', description: '简洁图标风格' }
  ];

  constructor() { }

  /**
   * 生成头像URL
   * @param style 头像样式名称
   * @param options 头像选项
   * @param format 图片格式 (svg, png, jpg, webp, avif)
   * @returns 头像URL
   */
  generateAvatarUrl(style: string, options: DiceBearOptions = {}, format: string = 'svg'): string {
    const params = new URLSearchParams();
    
    // 添加选项参数
    Object.entries(options).forEach(([key, value]) => {
      if (value !== undefined && value !== null) {
        if (Array.isArray(value)) {
          params.append(key, value.join(','));
        } else {
          params.append(key, String(value));
        }
      }
    });

    const queryString = params.toString();
    const url = `${this.baseUrl}/${style}/${format}${queryString ? '?' + queryString : ''}`;
    
    return url;
  }

  /**
   * 生成随机头像选项
   * @param seed 种子值（可选，用于生成一致的头像）
   * @returns 随机头像选项
   */
  generateRandomOptions(seed?: string): DiceBearOptions {
    const options: DiceBearOptions = {};
    
    if (seed) {
      options.seed = seed;
    }

    // 使用DiceBear API官方支持的安全背景色
    const safeBackgrounds = [
      'b6e3f4', 'c0aede', 'd1d4f9', 'ffd5dc', 'ffdfbf',
      'ff9aa2', 'ffb3ba', 'ffdfba', 'ffffba', 'baffc9',
      'bae1ff', 'e6ccff', 'ffccf2', 'ffcccb', 'f0f0f0'
    ];
    options.backgroundColor = [safeBackgrounds[Math.floor(Math.random() * safeBackgrounds.length)]];

    // 简化随机属性，只使用最稳定的选项
    const randomChance = Math.random();
    
    if (randomChance > 0.8) {
      // 20% 概率添加随机属性，使用最稳定的选项
      const safeHairColors = ['black', 'blonde', 'brown', 'red'];
      const safeSkinColors = ['tanned', 'pale', 'light', 'brown'];
      
      if (Math.random() > 0.5) {
        options.hairColor = [safeHairColors[Math.floor(Math.random() * safeHairColors.length)]];
      }
      if (Math.random() > 0.5) {
        options.skinColor = [safeSkinColors[Math.floor(Math.random() * safeSkinColors.length)]];
      }
    }

    return options;
  }

  // 积极向上的表情选项 - 只保留友好、开心的表情
  private readonly positiveExpressions = {
    // 友好的眼睛样式
    eyes: ['default', 'happy', 'hearts', 'side', 'wink', 'winkWacky'],
    // 积极的嘴巴样式 - 微笑、开心
    mouth: ['default', 'smile', 'twinkle'],
    // 友好的眉毛
    eyebrows: ['default', 'defaultNatural', 'flatNatural', 'raisedExcited', 'raisedExcitedNatural', 'upDown', 'upDownNatural']
  };

  // 商务休闲服装选项
  private readonly businessCasualClothing = {
    // 商务/休闲服装类型
    clothing: ['blazerAndShirt', 'blazerAndSweater', 'collarAndSweater', 'shirtCrewNeck', 'shirtScoopNeck', 'shirtVNeck'],
    // 专业的服装颜色 - 深蓝、灰色、黑色、白色等商务色
    clothingColor: ['262e33', '3c4f5c', '65c9ff', '5199e4', '25557c', '929598', 'a7ffc4', 'e6e6e6', 'ffffff']
  };

  /**
   * 获取积极表情的URL参数
   */
  private getPositiveExpressionParams(): string {
    const eyes = this.positiveExpressions.eyes[Math.floor(Math.random() * this.positiveExpressions.eyes.length)];
    const mouth = this.positiveExpressions.mouth[Math.floor(Math.random() * this.positiveExpressions.mouth.length)];
    const eyebrows = this.positiveExpressions.eyebrows[Math.floor(Math.random() * this.positiveExpressions.eyebrows.length)];
    
    return `&eyes=${eyes}&mouth=${mouth}&eyebrows=${eyebrows}`;
  }

  /**
   * 获取商务休闲服装的URL参数
   */
  private getBusinessCasualParams(): string {
    const clothing = this.businessCasualClothing.clothing[Math.floor(Math.random() * this.businessCasualClothing.clothing.length)];
    const clothingColor = this.businessCasualClothing.clothingColor[Math.floor(Math.random() * this.businessCasualClothing.clothingColor.length)];
    
    return `&clothing=${clothing}&clothingColor=${clothingColor}`;
  }

  /**
   * 根据用户名生成头像
   * @param username 用户名
   * @param style 头像样式
   * @returns 头像URL
   */
  generateAvatarForUser(username: string, style: string = 'avataaars'): string {
    // 使用用户名作为seed，确保同一用户总是得到相同的头像
    const seed = username.toLowerCase().replace(/[^a-z0-9]/g, '');
    
    // 基础URL
    let url = `${this.baseUrl}/${style}/svg?seed=${encodeURIComponent(seed)}`;
    
    // 对 avataaars 风格添加积极表情和商务休闲服装参数
    if (style === 'avataaars') {
      // 使用seed生成一致的表情和服装（基于用户名hash）
      const hash = seed.split('').reduce((a, b) => ((a << 5) - a) + b.charCodeAt(0), 0);
      const eyesIdx = Math.abs(hash) % this.positiveExpressions.eyes.length;
      const mouthIdx = Math.abs(hash >> 4) % this.positiveExpressions.mouth.length;
      const eyebrowsIdx = Math.abs(hash >> 8) % this.positiveExpressions.eyebrows.length;
      const clothingIdx = Math.abs(hash >> 12) % this.businessCasualClothing.clothing.length;
      const clothingColorIdx = Math.abs(hash >> 16) % this.businessCasualClothing.clothingColor.length;
      
      url += `&eyes=${this.positiveExpressions.eyes[eyesIdx]}`;
      url += `&mouth=${this.positiveExpressions.mouth[mouthIdx]}`;
      url += `&eyebrows=${this.positiveExpressions.eyebrows[eyebrowsIdx]}`;
      url += `&clothing=${this.businessCasualClothing.clothing[clothingIdx]}`;
      url += `&clothingColor=${this.businessCasualClothing.clothingColor[clothingColorIdx]}`;
    }
    
    return url;
  }

  /**
   * 生成多个头像选项供用户选择
   * @param count 生成数量
   * @param style 头像样式
   * @returns 头像URL数组
   */
  generateAvatarOptions(count: number = 6, style: string = 'avataaars'): string[] {
    const avatars: string[] = [];
    
    for (let i = 0; i < count; i++) {
      // 生成随机seed
      const timestamp = Date.now();
      const random = Math.random().toString(36).substring(2, 15);
      const seed = `avatar-${i}-${timestamp}-${random}`;
      
      // 基础URL
      let url = `${this.baseUrl}/${style}/svg?seed=${encodeURIComponent(seed)}`;
      
      // 对 avataaars 风格添加积极表情和商务休闲服装参数
      if (style === 'avataaars') {
        url += this.getPositiveExpressionParams();
        url += this.getBusinessCasualParams();
      }
      
      avatars.push(url);
    }
    
    return avatars;
  }

  /**
   * 获取头像样式的预览
   * @param style 头像样式
   * @returns 预览头像URL
   */
  getStylePreview(style: string): string {
    return this.generateAvatarUrl(style, { seed: 'preview' });
  }
}
