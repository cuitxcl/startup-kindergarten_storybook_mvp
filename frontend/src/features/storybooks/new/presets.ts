import imgPixar from "../../../assets/style-presets/pixar.jpg";
import imgGhibli from "../../../assets/style-presets/ghibli.jpg";
import imgWatercolor from "../../../assets/style-presets/watercolor.jpg";
import imgPencil from "../../../assets/style-presets/pencil.jpg";
import imgInk from "../../../assets/style-presets/ink.jpg";
import imgCartoon from "../../../assets/style-presets/cartoon.jpg";
import imgGongbi from "../../../assets/style-presets/gongbi.jpg";
import imgFlat from "../../../assets/style-presets/flat.jpg";
import imgCrayon from "../../../assets/style-presets/crayon.jpg";
import imgPapercut from "../../../assets/style-presets/papercut.jpg";
import imgClay from "../../../assets/style-presets/clay.jpg";
import imgOilpaint from "../../../assets/style-presets/oilpaint.jpg";

// 画面风格预设：每种画风配一张代表性预览图，选中后生成插图时会严格按该画风绘制。
export const STYLE_PRESETS = [
  { label: "皮克斯3D", tag: "立体动画感", image: imgPixar, value: "画面风格：皮克斯3D动画风。高质量3D渲染，角色立体圆润、毛发细腻、大眼睛表情生动，柔和影棚光，色彩鲜艳明快，像动画电影截图。" },
  { label: "宫崎骏", tag: "吉卜力手绘", image: imgGhibli, value: "画面风格：宫崎骏/吉卜力手绘风。手绘水彩质感，背景细腻丰富，天空云朵柔和，光影温暖治愈，带淡淡怀旧气息。" },
  { label: "水彩绘本", tag: "清新通透", image: imgWatercolor, value: "画面风格：水彩绘本风。半透明水彩晕染，边缘柔和，留白自然，可见纸张纹理，色调清淡通透，经典童书插画感。" },
  { label: "手绘彩铅", tag: "细腻笔触", image: imgPencil, value: "画面风格：手绘彩铅风。可见铅笔排线和叠色笔触，质感温暖手工感强，色调柔和，像老师亲手绘制的插画。" },
  { label: "水墨画", tag: "东方韵味", image: imgInk, value: "画面风格：中国水墨画风。毛笔笔触灵动，墨色浓淡变化，点缀淡雅色彩，留白有意境，宣纸质感，东方美学。" },
  { label: "卡通", tag: "简洁明快", image: imgCartoon, value: "画面风格：卡通插画风。粗描边、平涂鲜艳色块，造型简洁夸张，表情生动可爱，现代儿童动画感，画面干净明快。" },
  { label: "国风工笔画", tag: "精致典雅", image: imgGongbi, value: "画面风格：国风工笔画风。线条细腻工整，层层晕染的矿物色彩，构图典雅精致，传统国画质感，适合文化主题。" },
  { label: "扁平插画", tag: "现代简约", image: imgFlat, value: "画面风格：扁平插画风。极简几何造型，大面积纯色块，无渐变无阴影，配色时尚，现代儿童读物设计感。" },
  { label: "蜡笔油画棒", tag: "童趣涂鸦", image: imgCrayon, value: "画面风格：蜡笔油画棒风。厚重蜡笔质感，色彩浓艳大胆，笔触稚拙童趣，像孩子自己的涂鸦作品，纸纹明显。" },
  { label: "剪纸拼贴", tag: "手工纸艺", image: imgPapercut, value: "画面风格：剪纸拼贴风。层叠剪纸造型，可见纸张边缘和投影，手工纸艺质感，色彩对比鲜明，民间艺术趣味。" },
  { label: "黏土定格", tag: "手作立体", image: imgClay, value: "画面风格：黏土定格动画风。角色像手工捏制的黏土，带指纹纹理，材质柔软立体，柔和布光，阿德曼动画式手作魅力。" },
  { label: "厚涂油画", tag: "艺术质感", image: imgOilpaint, value: "画面风格：厚涂油画风。可见丰富笔触和颜料堆叠质感，色彩浓郁温暖，光影细腻，美术馆级绘本封面感。" },
];

// 故事风格预设：情节基调与叙事类型，决定故事怎么走；与画面风格（怎么画）互相独立，可自由组合。
export const STORY_STYLE_PRESETS = [
  { label: "日常温情型", tag: "治愈系", value: "日常温情治愈系：以幼儿园真实生活为底色，节奏舒缓，情节小而暖，老师温柔引导，结尾有拥抱感的小收获。" },
  { label: "冒险奇幻型", tag: "想象力爆棚", value: "冒险奇幻型：魔法森林、发光生物、会说话的伙伴，画面瑰丽夸张，情节一波三折，鼓励孩子勇敢探索。" },
  { label: "幽默搞笑型", tag: "笑点不断", value: "幽默搞笑型：夸张的误会和反转情节，角色表情动作喜剧化，让孩子在笑声中明白一个小道理。" },
  { label: "科学探索型", tag: "满足好奇心", value: "科学探索型：从孩子的一个小疑问出发，经历观察、猜想、验证的过程，认识自然和生活中的科学现象。" },
  { label: "成长引导型", tag: "规则与习惯", value: "成长引导型：聚焦规则、习惯和社交情景，冲突真实、解决办法具体，老师示范清晰，适合班级共读讨论。" },
  { label: "自然认知型", tag: "亲近自然", value: "自然认知型：以四季、动植物、天气变化为线索，画面清新细腻，在观察自然的过程中渗透认知目标。" },
  { label: "睡前安恬型", tag: "温柔入眠", value: "睡前安恬型：节奏轻缓、画面柔和，情节安稳温暖，以晚安和拥抱收尾，帮助孩子平静下来进入睡眠。" },
  { label: "节日民俗型", tag: "传统节庆", value: "节日民俗型：围绕传统节日与民俗活动展开，融入灯笼、舞龙、美食等节庆元素，热闹喜庆，渗透文化认知。" },
  { label: "海洋探险型", tag: "海底世界", value: "海洋探险型：潜入五彩斑斓的海底世界，认识海洋动物朋友，情节新奇有趣，传递保护海洋的小意识。" },
  { label: "太空科幻型", tag: "宇宙奇想", value: "太空科幻型：乘坐飞船遨游宇宙，遇见星球、机器人和外星人朋友，画面充满未来感，激发对宇宙的好奇。" },
  { label: "复古民间故事型", tag: "经典回味", value: "复古民间故事型：采用民间故事叙事方式，有老故事的韵味和寓意，画面带传统质朴质感，适合品格启蒙。" },
  { label: "动物拟人型", tag: "萌趣动物村", value: "动物拟人型：动物角色穿衣说话、上学上班，像小朋友一样生活和交朋友，萌趣可爱，贴近孩子的同伴世界。" },
];
