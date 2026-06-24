# 图片生成 Prompt 规范

## 目标

定义儿童定制读本项目中的图片生成输入规范，确保：
- 角色设定稳定
- 重复道具设定稳定
- Prompt 结构固定
- 页面变量可控
- 不同开发实现保持一致

这份文档面向产品、后端和提示词工程。

## 设计原则

- 角色信息必须结构化
- 重复道具信息必须结构化
- 风格信息必须枚举化
- 页面信息必须拆成可控字段
- Prompt 模板必须固定
- 每次生成尽量只替换少量变量

## 生成链路

图片生成统一依赖四类输入：

1. `character_profile`
2. `parent_character_profiles`
3. `prop_profiles`
4. `scene_spec`
5. `style_spec`

最终由模板系统拼装成模型可用 Prompt。

## 1. character_profile

### 作用

定义主角的固定属性，作为整本读本的角色锚点。

这里的 `character_profile` 主要指儿童卡通人物形象设定，而不是简单头像。

### JSON 结构

```json
{
  "character_id": "char_001",
  "name": "小雨",
  "nickname": "雨雨",
  "age_group": "5岁",
  "gender_expression": "女孩",
  "hair": "黑色短发，齐刘海",
  "skin_tone": "自然偏白",
  "face_shape": "圆脸",
  "body_proportion": "幼儿比例，头稍大",
  "outfit_top": "黄色卫衣",
  "outfit_bottom": "蓝色背带裤",
  "shoe": "白色运动鞋",
  "accessory": "小兔子发夹",
  "signature_colors": ["黄色", "蓝色"],
  "interest_elements": ["积木", "小兔子"],
  "visual_must_keep": [
    "齐刘海",
    "黄色卫衣",
    "小兔子发夹"
  ],
  "negative_rules": [
    "不要写实",
    "不要成人化五官",
    "不要复杂光影",
    "不要额外主角"
  ],
  "reference_image_id": "img_ref_001"
}
```

### 字段说明

- `character_id`
  - 角色唯一标识
- `name`
  - 正式姓名，用于内部记录
- `nickname`
  - 可用于文案或标题中的儿童称呼
- `age_group`
  - 推荐使用离散值，例如 `4岁`、`5岁`、`6岁`
- `gender_expression`
  - 用于稳定角色表现，不等于强制性别刻板表达
- `hair`
  - 发型描述，必须是简洁可视觉化的描述
- `skin_tone`
  - 肤色描述，避免模糊词
- `face_shape`
  - 可选字段，用于增强角色一致性
- `body_proportion`
  - 固定幼儿比例
- `outfit_top` / `outfit_bottom` / `shoe`
  - 衣着描述，优先保留颜色和款式
- `accessory`
  - 标志性配件
- `signature_colors`
  - 用于程序侧做一致性校验
- `interest_elements`
  - 用于在背景或细节中轻度体现孩子偏好
- `visual_must_keep`
  - 不允许漂移的关键特征
- `negative_rules`
  - 角色级负面约束
- `reference_image_id`
  - 标准角色图 ID

### 约束

- 必须至少有 3 个 `visual_must_keep`
- 必须有 `reference_image_id`
- 发型、服装主色、配件三类中至少两类不可缺失

## 2. parent_character_profile

### 作用

定义家长角色的卡通人物形象，用于家庭场景、陪伴阅读场景和节日纪念场景。

### JSON 结构

```json
{
  "parent_character_id": "parent_char_001",
  "role": "妈妈",
  "name": "妈妈",
  "hair": "棕色中长发",
  "skin_tone": "自然肤色",
  "face_shape": "温和圆脸",
  "body_proportion": "成人卡通比例，亲和感",
  "outfit_top": "米色针织衫",
  "outfit_bottom": "浅蓝色长裙",
  "accessory": "细框眼镜",
  "visual_must_keep": [
    "棕色中长发",
    "米色针织衫",
    "细框眼镜"
  ],
  "negative_rules": [
    "不要写实",
    "不要过度成熟化",
    "不要夸张时尚大片风格"
  ],
  "reference_image_id": "img_parent_ref_001"
}
```

### 使用规则

- 仅在家庭场景、家园沟通场景或节日纪念场景中启用
- 不要求每本内容都出现家长角色
- 出现时应保持与家长设定一致

## 3. style_spec

### 作用

定义固定画风，避免风格随页面漂移。

### JSON 结构

```json
{
  "style_id": "storybook_flat_v1",
  "name": "温暖扁平绘本风",
  "prompt_style": [
    "温暖儿童绘本插画",
    "扁平风格",
    "柔和配色",
    "简洁背景",
    "清晰表情"
  ],
  "negative_style": [
    "不要写实照片风",
    "不要电影级光影",
    "不要复杂透视",
    "不要背景过满"
  ],
  "color_tone": "warm_soft",
  "line_density": "low",
  "detail_level": "medium_low",
  "background_complexity": "low"
}
```

### 建议首期风格枚举

- `storybook_flat_v1`
- `storybook_crayon_v1`
- `storybook_collage_v1`

### 约束

- 首期最多开放 3 种风格
- 风格不能允许教师自由填写
- 每个风格必须有独立负面约束

## 4. prop_profile

### 作用

定义会在多页重复出现的关键道具，作为整本读本的道具锚点。

### JSON 结构

```json
{
  "prop_id": "prop_001",
  "name": "小兔子玩偶",
  "shape": "圆头长耳朵",
  "primary_color": "奶白色",
  "secondary_color": "浅粉色耳朵内侧",
  "material_style": "毛绒玩偶",
  "size": "适合幼儿抱在怀里",
  "visual_must_keep": [
    "长耳朵",
    "奶白色",
    "粉色耳朵内侧"
  ],
  "negative_rules": [
    "不要变成真实兔子",
    "不要变成卡通人物",
    "不要频繁改变颜色"
  ],
  "reference_image_id": "img_prop_ref_001"
}
```

### 字段说明

- `prop_id`
  - 道具唯一标识
- `name`
  - 道具名称
- `shape`
  - 关键轮廓描述
- `primary_color` / `secondary_color`
  - 主辅色
- `material_style`
  - 材质感，例如毛绒、塑料、木制
- `size`
  - 相对大小描述
- `visual_must_keep`
  - 不允许漂移的关键特征
- `negative_rules`
  - 道具级负面约束
- `reference_image_id`
  - 道具参考图 ID，可选但推荐

### 何时需要 `prop_profile`

满足任一条件就建议建立：
- 同一本道具出现 2 次以上
- 道具与主角身份强相关
- 道具承担剧情推进作用
- 家长或老师会关注它是否一致

## 5. scene_spec

### 作用

定义某一页的场景变量。

### JSON 结构

```json
{
  "scene_id": "scene_page_03",
  "page_number": 3,
  "location": "幼儿园教室",
  "action": "把积木递给同学",
  "emotion": "开心，主动分享",
  "composition": "中景，主角位于画面中央偏左",
  "camera_angle": "平视",
  "parent_characters": [
    {
      "parent_character_id": "parent_char_001",
      "role": "妈妈",
      "importance": "medium",
      "placement": "主角右侧"
    }
  ],
  "props": [
    {
      "prop_id": "prop_001",
      "importance": "high",
      "placement": "主角怀里"
    }
  ],
  "supporting_characters": [
    {
      "role": "同学",
      "count": 1,
      "importance": "low"
    }
  ],
  "background_elements": [
    "玩具柜",
    "地垫",
    "少量积木"
  ],
  "teaching_focus": "分享合作",
  "must_show": [
    "主角主动递出积木",
    "主角表情开心"
  ],
  "must_avoid": [
    "背景拥挤",
    "其他角色比主角更显眼"
  ]
}
```

### 字段约束

- `location` 必填
- `action` 必填
- `emotion` 必填
- `composition` 必填
- `parent_characters` 可为空，若出现则应引用已定义的家长角色设定
- `props` 可为空，但如果某个道具重复出现，优先使用 `prop_id` 引用已定义道具卡
- `supporting_characters` 可为空，但若有，重要性必须低于主角
- `background_elements` 不建议超过 5 项

## 6. Prompt 模板结构

### 标准模板

```text
你要绘制一张儿童绘本插画。

固定角色设定：
- 主角：{age_group}{gender_expression}，名字是{name}
- 发型：{hair}
- 肤色：{skin_tone}
- 脸型：{face_shape}
- 比例：{body_proportion}
- 上装：{outfit_top}
- 下装：{outfit_bottom}
- 鞋子：{shoe}
- 配件：{accessory}
- 必须保留的视觉特征：{visual_must_keep}

家长角色设定：
- 家长角色：{parent_roles}
- 家长角色细节：{parent_character_details}
- 家长角色必须保留的特征：{parent_visual_must_keep}

固定道具设定：
- 高频关键道具：{prop_names}
- 道具细节：{prop_details}
- 道具必须保留的特征：{prop_visual_must_keep}

固定风格设定：
- 风格：{style_name}
- 风格关键词：{prompt_style}
- 背景复杂度：{background_complexity}
- 细节等级：{detail_level}

当前页面设定：
- 页码：第{page_number}页
- 场景：{location}
- 动作：{action}
- 情绪：{emotion}
- 构图：{composition}
- 镜头视角：{camera_angle}
- 家长角色：{parent_characters}
- 关键道具：{props}
- 配角：{supporting_characters}
- 背景元素：{background_elements}
- 教学重点：{teaching_focus}
- 必须出现：{must_show}

一致性要求：
- 主角必须与参考角色图保持一致
- 不得改变主角的发型、服装主色和标志性配件
- 如果出现家长角色，不得改变家长卡通人物形象的关键特征
- 不得改变重复关键道具的颜色、形状和材质感
- 主角必须是画面主视觉中心

负面要求：
- 角色负面要求：{character_negative_rules}
- 家长角色负面要求：{parent_character_negative_rules}
- 道具负面要求：{prop_negative_rules}
- 风格负面要求：{negative_style}
- 页面负面要求：{must_avoid}
```

## 7. Prompt 组装规则

### 规则 1

固定字段顺序，不允许不同服务各自拼装顺序。

### 规则 2

数组字段进入 Prompt 前需格式化为短列表文本，不直接输出原始 JSON。

例如：

- `["齐刘海", "黄色卫衣", "小兔子发夹"]`

应格式化为：

- `齐刘海；黄色卫衣；小兔子发夹`

### 规则 3

空字段不应原样渲染为 `null` 或空字符串，而应跳过该行。

### 规则 4

页面变量描述不要过长。单个字段建议控制在一句话内。

### 规则 5

如果本页没有重复关键道具，`固定道具设定` 整段可省略。

### 规则 6

如果本页引用了多个关键道具，应优先保留 1 到 2 个最重要的道具描述，避免 Prompt 过长。

### 规则 7

如果本页不出现家长角色，`家长角色设定` 整段可省略。

## 8. 参考图使用规范

### 标准角色图要求

- 清晰展示主角发型、服装和配件
- 背景简单
- 无复杂动作
- 不包含抢眼配角

### 生成页图时的要求

- 每次都传入 `reference_image_id`
- 如出现家长角色，也应传入对应 `parent reference_image_id`
- 如有道具参考图，也应传入对应 `prop reference_image_id`
- 如模型支持参考图权重，应为角色一致性设置固定权重
- 不同服务不得自行省略参考图

## 9. 重试策略

### 允许单页重试

重试时优先调整：
- 构图描述
- 配角数量
- 背景元素数量

不要优先调整：
- 主角核心特征
- 家长角色核心特征
- 道具核心特征
- 风格模板

### 重试次数建议

- 单页默认最多重试 2 次
- 超过 2 次进入人工复核或降级策略

## 10. 降级策略

当稳定性不足时，优先做以下降级：

1. 减少背景元素
2. 减少配角数量
3. 减少非关键道具描述
4. 使用更简单构图
5. 降低细节层级
6. 回退到默认稳定风格

## 11. 版本管理

以下对象必须版本化：

- `character_profile_version`
- `parent_character_profile_version`
- `prop_profile_version`
- `style_spec_version`
- `scene_spec_version`
- `prompt_template_version`

任何一次效果变化，都必须能追溯到具体版本。

## 12. 验收标准

一套 Prompt 规范是否可用，至少看下面几点：

- 同一角色跨 6 到 10 页是否能保持一致
- 同一家长角色在家庭场景中是否能保持一致
- 同一关键道具跨多页是否能保持一致
- 不同页面是否仍保持统一画风
- 单页首次生成通过率是否提升
- 单页平均重试次数是否下降
- 单本读本的平均图片成本是否下降
