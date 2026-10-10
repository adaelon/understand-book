import { useEffect, useState, useSyncExternalStore } from "react";
import { Link, Outlet, useLocation, useNavigate } from "@tanstack/react-router";
import { BookOpen, Users, Moon, Sun, ChartColumn, Ticket } from "lucide-react";
import { login, logout, session, startIdentitySync } from "./lib/api";
import { useTheme } from "./context/theme-provider";
import { Button } from "./components/ui/button";
import { Input } from "./components/ui/input";
import { Main } from "./components/layout/main";
import {
  Sidebar,
  SidebarContent,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
} from "./components/ui/sidebar";
import { Field, OperationsProvider } from "./components/operation";

export function App() {
  const auth = useSyncExternalStore(session.subscribe, session.snapshot);
  const navigate = useNavigate();
  const location = useLocation();
  const pathname = location.pathname.replace(/^\/admin(?=\/|$)/, "") || "/";
  const [returnTo, setReturnTo] = useState(
    pathname === "/login" ? "/users" : pathname,
  );
  const [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  const { theme, setTheme } = useTheme();
  useEffect(startIdentitySync, []);
  useEffect(() => {
    if (auth.status !== "ready") return;
    if (!auth.identity && pathname !== "/login") {
      setReturnTo(pathname);
      void navigate({ to: "/login", replace: true });
    } else if (auth.identity?.capabilities.admin && pathname === "/login")
      void navigate({ to: returnTo, replace: true });
  }, [auth.status, auth.identity, pathname, navigate, returnTo]);
  async function signOut() {
    setBusy(true);
    setError("");
    try {
      await logout();
    } catch (failure) {
      setError((failure as Error).message);
    } finally {
      setBusy(false);
    }
  }
  if (auth.status === "checking")
    return (
      <main className="p-10" role="status">
        正在确认登录状态…
      </main>
    );
  if (auth.status === "error")
    return (
      <main className="p-10">
        <p role="alert">{auth.error}</p>
        <Button onClick={() => void session.refresh()}>重试连接</Button>
      </main>
    );
  if (!auth.identity)
    return (
      <main className="mx-auto flex min-h-svh max-w-sm flex-col justify-center p-6">
        <BookOpen className="mb-6 size-9" />
        <h1>登录运营后台</h1>
        <p className="my-3 text-sm text-muted-foreground">
          使用现有阅读账号，管理权限由服务确认。
        </p>
        <form
          onSubmit={async (event) => {
            event.preventDefault();
            const form = event.currentTarget;
            const fields = new FormData(form);
            setBusy(true);
            setError("");
            try {
              await login(
                String(fields.get("username")),
                String(fields.get("password")),
              );
            } catch {
              setError("登录失败，请核对账号、密码或连接。");
              await session.refresh();
            } finally {
              form.reset();
              setBusy(false);
            }
          }}
        >
          <Field label="邮箱或账号">
            <Input name="username" autoComplete="username" required />
          </Field>
          <Field label="密码">
            <Input
              name="password"
              type="password"
              autoComplete="current-password"
              required
            />
          </Field>
          <Button type="submit" className="my-3 w-full" disabled={busy}>
            登录
          </Button>
        </form>
        <p role="alert" className="text-destructive">
          {error}
        </p>
        <a href="/" className="mt-4 text-sm underline">
          返回阅读
        </a>
      </main>
    );
  if (!auth.identity.capabilities.admin)
    return (
      <main className="mx-auto max-w-md space-y-5 p-10">
        <h1>无管理权限</h1>
        <p>{auth.identity.email || auth.identity.user_id} 可以从阅读入口访问自己的材料。</p>
        <Button asChild>
          <a href="/">返回阅读</a>
        </Button>{" "}
        <Button
          variant="outline"
          disabled={busy}
          onClick={() => void signOut()}
        >
          退出登录
        </Button>
        <p role="alert">{error}</p>
      </main>
    );
  return (
    <SidebarProvider>
      <Sidebar>
        <SidebarHeader className="p-5">
          <span className="flex items-center gap-2 font-semibold">
            <BookOpen /> Understand Book
          </span>
          <span className="text-xs text-muted-foreground">运营后台</span>
        </SidebarHeader>
        <SidebarContent className="p-3">
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton asChild isActive={pathname.startsWith('/users')}>
                <Link to="/users">
                  <Users />
                  <span>账号管理</span>
                </Link>
              </SidebarMenuButton>
            </SidebarMenuItem>
            <SidebarMenuItem><SidebarMenuButton asChild isActive={pathname === '/usage'}><Link to="/usage"><ChartColumn /><span>全站用量</span></Link></SidebarMenuButton></SidebarMenuItem>
            <SidebarMenuItem><SidebarMenuButton asChild isActive={pathname === '/invites'}><Link to="/invites"><Ticket /><span>内测码</span></Link></SidebarMenuButton></SidebarMenuItem>
          </SidebarMenu>
        </SidebarContent>
      </Sidebar>
      <SidebarInset>
        <header className="flex min-h-16 flex-wrap items-center gap-3 border-b px-4">
          <SidebarTrigger />
          <span className="mr-auto break-all text-sm">{auth.identity.email || auth.identity.user_id}</span>
          <Button
            variant="ghost"
            size="icon"
            aria-label="切换主题"
            onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
          >
            {theme === "dark" ? <Sun /> : <Moon />}
          </Button>
          <Button asChild variant="outline">
            <a href="/">返回阅读</a>
          </Button>
          <Button
            variant="ghost"
            disabled={busy}
            onClick={() => void signOut()}
          >
            退出登录
          </Button>
        </header>
        <Main className="mx-auto w-full max-w-7xl space-y-6">
          <p role="alert">{error}</p>
          <OperationsProvider key={auth.epoch}>
            <Outlet />
          </OperationsProvider>
        </Main>
      </SidebarInset>
    </SidebarProvider>
  );
}
