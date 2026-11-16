export type ProductTourContext = 'library' | 'boards' | 'reader' | 'board';
export type ProductTourStep = {
  id: string;
  target: string;
  title: string;
  body: string;
  hint?: string;
  next?: string;
  action?: 'boards' | 'create-board' | 'input';
  demo?: boolean;
  optional?: boolean;
};

const thoughts: ProductTourStep = {
  id: 'thoughts', target: '[data-tour="thought-entry"], [data-tour="thought-window"]',
  title: '把自己的想法留下来',
  body: '疑问、猜测和自己的解释，都值得记录。用“记一下”保存原话，以后可以补充、修订，或交给 Agent 继续推敲。',
  hint: '不必先想清楚，也不必写得完整。', optional: true,
};
const boards: ProductTourStep = {
  id: 'boards-entry', target: '[data-tour="boards-entry"]', title: '学习画板，从这里进入',
  body: '学习画板和学习资料并列。你可以直接进入画板首页，新建一个问题，或继续整理之前的理解。',
  hint: '阅读中、画板中和手机上，都能回到这个入口。', optional: true,
};

export const productTourSteps: Record<ProductTourContext, ProductTourStep[]> = {
  library: [
    { id: 'purpose', target: '[data-tour="learning-purpose"]', title: '这里承载你的理解，也陪它继续发展',
      body: '从你想弄懂的问题开始，把资料、疑问和自己的解释留在这里。Agent 会结合材料陪你推敲，让理解逐渐变得清楚、彼此连接。',
      hint: '你的想法，是学习的起点。' },
    { id: 'materials', target: '[data-tour="materials-entry"]', title: '从资料开始，带着问题去读',
      body: '在学习资料里建立工作区，放入书籍或视频。打开材料后，可以直接告诉 Agent 想理解什么，它会查阅内容、组织解释，并根据你的回应继续。' },
    thoughts,
    { ...boards, optional: false, next: '看看学习画板', action: 'boards' },
  ],
  boards: [
    { id: 'board-uses', target: '[data-tour="board-uses"]', title: '把一个想弄懂的问题展开',
      body: '画板可以帮你梳理过程与关系、比较容易混淆的概念，也可以检查自己的理解。资料和你的想法能放在一起，继续讨论。',
      hint: '例如：“这两个概念看起来很像，区别到底在哪里？”' },
    { id: 'board-library', target: '[data-tour="board-library"]', title: '之前的画板，都在这里继续',
      body: '创建过的画板会出现在这里。可以按问题名称、资料或工作区查找，打开后回到原来的内容与讨论。' },
    { id: 'board-create', target: '[data-tour="board-create"]', title: '从你自己的问题开始',
      body: '给想弄懂的问题起个名字，再选一份相关资料。进入画板后，直接说出你的想法，就能和 Agent 一起梳理。',
      next: '试着创建一张画板', action: 'create-board' },
  ],
  reader: [
    { id: 'agent', target: '[data-tour="reader-agent"]', title: '这是能行动的学习 Agent',
      body: '告诉它你想弄懂什么。它会按需要查阅材料、组织解释、制作演示，再根据你的回应调整下一步。你的理解会成为这段学习的线索。',
      hint: '例如：“我觉得这里是这个意思，但不确定这个条件有什么用。”' },
    { id: 'presentation', target: '[data-tour="presentation"]', title: '有些理解，适合动手看',
      body: '打开“演示页”，再描述想弄懂的问题，Agent 可以把它制作成可交互的讲解。改变条件、观察结果，再继续追问。',
      hint: '点开下面的示例，感受一下演示页。', demo: true },
    boards,
    thoughts,
    { id: 'reader-input', target: '[data-tour="reader-input"]', title: '现在，说一点你自己的想法',
      body: '说说你已经怎么理解、哪里拿不准，或者想到了什么反例。表达得越具体，Agent 就越能沿着你的思路帮助你。',
      hint: '“我的理解是……但我还不确定……”', next: '开始表达我的想法', action: 'input' },
  ],
  board: [
    { id: 'board-canvas', target: '[data-tour="board-canvas"]', title: '让自己的理解在画板上展开',
      body: '把概念、资料、图解和自己的想法放在一起，整理位置、连接关系。选中一个对象，还可以围绕它继续讲解或补充例子。',
      hint: '画板会保存下来，供你回看和继续发展。' },
    { id: 'board-agent', target: '[data-tour="board-agent"]', title: '边讨论，边把关系画清楚',
      body: '这里也是 Agent。你可以请它梳理主题、画出关系，或检查当前的解释。选中的对象会成为讨论的上下文。' },
    thoughts,
    { id: 'board-input', target: '[data-tour="board-input"]', title: '从一句不完整的理解开始',
      body: '把你现在的解释、困惑或反例写在这里。你可以纠正 Agent，也可以请它对照资料，检查遗漏了哪些条件。',
      hint: '“我的理解是……这样理解漏掉了什么？”', next: '写下我的想法', action: 'input' },
  ],
};

export function tourStorageKey(owner: string, context: ProductTourContext) {
  return `understand-book.product-tour.v1.${encodeURIComponent(owner)}.${context}`;
}

export type TourRect = { x: number; y: number; width: number; height: number };
const clamp = (n: number, min: number, max: number) => Math.max(min, Math.min(n, max));

/** Position beside the real element; crop the spotlight for a canvas taller than the available screen. */
export function placeTour(target: TourRect, viewport: { width: number; height: number }, card: { width: number; height: number }) {
  const margin = 12, gap = 14, padding = 6;
  const left = clamp(target.x - padding, 4, viewport.width - 4);
  const top = clamp(target.y - padding, 4, viewport.height - 4);
  const right = clamp(target.x + target.width + padding, left, viewport.width - 4);
  const bottom = clamp(target.y + target.height + padding, top, viewport.height - 4);
  const hole = { x: left, y: top, width: right - left, height: bottom - top };
  let x = clamp(left, margin, viewport.width - card.width - margin);
  let y: number;
  if (viewport.width - right >= card.width + gap + margin) {
    x = right + gap; y = clamp(top, margin, viewport.height - card.height - margin);
  } else if (left >= card.width + gap + margin) {
    x = left - gap - card.width; y = clamp(top, margin, viewport.height - card.height - margin);
  } else if (top >= card.height + gap + margin) {
    y = top - card.height - gap;
  } else if (viewport.height - bottom >= card.height + gap + margin) {
    y = bottom + gap;
  } else {
    y = viewport.height - card.height - margin;
    if (y - top >= 64) hole.height = Math.max(0, y - gap - top);
    else {
      y = margin;
      hole.y = Math.max(top, y + card.height + gap);
      hole.height = Math.max(0, bottom - hole.y);
    }
  }
  return { hole, card: { x, y } };
}
