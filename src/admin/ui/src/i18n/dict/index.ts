// 字典汇总导出
import { commonDict } from './common';
import { navDict } from './nav';
import { loginDict } from './login';
import { postsDict } from './posts';
import { settingsDict } from './settings';
import { contentDict } from './content';
import { setupDict } from './setup';

// 合并所有字典
export const dictionary = {
  zh: {
    ...commonDict.zh,
    ...navDict.zh,
    ...loginDict.zh,
    ...postsDict.zh,
    ...settingsDict.zh,
    ...contentDict.zh,
    ...setupDict.zh,
  },
  en: {
    ...commonDict.en,
    ...navDict.en,
    ...loginDict.en,
    ...postsDict.en,
    ...settingsDict.en,
    ...contentDict.en,
    ...setupDict.en,
  },
};

export type Dictionary = typeof dictionary.zh;
export type DictionaryKey = keyof Dictionary;
