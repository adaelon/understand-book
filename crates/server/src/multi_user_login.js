const storageKey = 'understand-book.locale';
const requested = new URLSearchParams(location.search).get('lang');
let language = requested === 'en' || requested === 'zh-CN' ? requested : 'zh-CN';
try {
  if (requested === 'en' || requested === 'zh-CN') localStorage.setItem(storageKey, language);
  else language = localStorage.getItem(storageKey) === 'en' ? 'en' : 'zh-CN';
} catch { /* Language links also work without browser storage. */ }
const text = (zh, en) => language === 'en' ? en : zh;
document.documentElement.lang = language;
document.title = text('Understand Book · 登录', 'Understand Book · Sign in');
if (language === 'en') for (const element of document.querySelectorAll('[data-en]')) element.textContent = element.dataset.en;
const selector = document.querySelector('#language');
selector.value = language;
selector.addEventListener('change', () => {
  const url = new URL(location.href); url.searchParams.set('lang', selector.value); location.assign(url.href);
});
const form = document.querySelector('#login');
const library = document.querySelector('#library');
const notice = document.querySelector('#notice');
const books = document.querySelector('#books');
let csrf = '';
let generation = 0;
function clearAccount() {
  generation += 1;
  csrf = '';
  books.replaceChildren();
  document.querySelector('#identity').textContent = '';
  library.hidden = true;
  form.hidden = false;
  form.reset();
}
async function request(path, body) {
  const response = await fetch(`/api${path}`, {
    method: body === undefined ? 'GET' : 'POST', credentials: 'same-origin', cache: 'no-store',
    headers: body === undefined ? {} : { 'Content-Type': 'application/json', 'X-CSRF-Token': csrf },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) {
    const error = new Error(response.status === 429 ? text('尝试过于频繁，请稍后再试。', 'Too many attempts. Please try again later.') : response.status === 401 ? text('账号或密码不正确，或登录已失效。', 'The account or password is incorrect, or your session has expired.') : text('请求未完成，请重试。', 'The request could not be completed. Please try again.'));
    error.status = response.status;
    throw error;
  }
  return response.json();
}
async function showAccount(identity) {
  const current = ++generation;
  csrf = identity.csrf_token;
  form.reset(); form.hidden = true; library.hidden = false;
  document.querySelector('#identity').textContent = identity.email || identity.user_id;
  let result;
  try { result = await request('/library'); }
  catch (error) {
    if (generation === current) { if (error.status === 401) clearAccount(); notice.textContent = error.message; }
    return;
  }
  if (generation !== current) return;
  books.replaceChildren();
  for (const book of result.books) {
    const item = document.createElement('li');
    item.textContent = book.published_book_ref.book_id;
    books.append(item);
  }
  document.querySelector('#empty').hidden = result.books.length !== 0;
  notice.textContent = '';
}
form.addEventListener('submit', async event => {
  event.preventDefault();
  const button = form.querySelector('button'); button.disabled = true;
  const data = new FormData(form);
  const current = ++generation;
  try {
    const identity = await request('/auth/login', { username: data.get('username'), password: data.get('password') });
    if (generation === current) await showAccount(identity);
  }
  catch (error) { if (generation === current) { if (error.status === 401) clearAccount(); notice.textContent = error.message; } }
  finally { form.elements.password.value = ''; button.disabled = false; }
});
document.querySelector('#logout').addEventListener('click', async () => {
  const current = generation;
  try { await request('/auth/logout', {}); if (generation === current) { clearAccount(); notice.textContent = text('已退出登录。', 'Signed out.'); } }
  catch (error) { if (generation === current) { if (error.status === 401) clearAccount(); notice.textContent = error.message; } }
});
window.addEventListener('pageshow', async () => {
  clearAccount();
  const current = generation;
  try { const identity = await request('/auth/me'); if (generation === current) await showAccount(identity); }
  catch { if (generation === current) clearAccount(); }
});
