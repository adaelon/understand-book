# 期末复习题解答

题号与正文末尾的“期末复习题”一致，以下给出计算与证明过程。

<!-- answer id="review-01" -->
## 例 1　凸函数的充要条件

以下三个条件都可用于不要求可微的实值函数。

1. **定义式。** 对任意 $x,y\in\mathbb R^n$、$t\in[0,1]$，有

$$f(tx+(1-t)y)\leq tf(x)+(1-t)f(y).$$

2. **上图判别。** 集合 $\operatorname{epi}f=\{(x,a):f(x)\leq a\}$ 是凸集，证明见例 3。

3. **限制到直线。** 对任意 $x,d\in\mathbb R^n$，一元函数 $\varphi(t)=f(x+td)$ 在 $\mathbb R$ 上凸。这与定义式等价：一方面，直线上两点的凸组合仍由参数的凸组合表示；另一方面，任意两点 $x,y$ 都在直线 $x+t(y-x)$ 上。

另外两种常用判别需要相应的可微条件：若 $f\in C^1$，凸性等价于

$$f(y)\geq f(x)+\nabla f(x)^T(y-x),\qquad\forall x,y;$$

若 $f\in C^2$，凸性等价于 $\nabla^2f(x)\succeq0$ 对所有 $x$ 成立。写这两种判别时，应同时写出对应的可微条件。
<!-- endanswer -->

<!-- answer id="review-02" -->
## 例 2　凸函数的水平集

任取 $x,y\in L(x_0)$、$t\in[0,1]$，有 $f(x)\leq f(x_0)$、$f(y)\leq f(x_0)$。利用 $f$ 的凸性：

$$f(tx+(1-t)y)\leq tf(x)+(1-t)f(y)
\leq tf(x_0)+(1-t)f(x_0)=f(x_0).$$

所以 $tx+(1-t)y\in L(x_0)$，按凸集定义，$L(x_0)$ 为凸集。
<!-- endanswer -->

<!-- answer id="review-03" -->
## 例 3　上图判别法

*另见第 3 章作业题 3。*

**先证必要性。** 设 $f$ 凸。取 $(x,a),(y,b)\in\operatorname{epi}f$、$t\in[0,1]$，有

$$f(tx+(1-t)y)\leq tf(x)+(1-t)f(y)\leq ta+(1-t)b.$$

因此 $(tx+(1-t)y,ta+(1-t)b)\in\operatorname{epi}f$，上图为凸集。

**再证充分性。** 设上图为凸集。因为 $(x,f(x))$、$(y,f(y))$ 都在上图内，其凸组合也在上图内，所以

$$f(tx+(1-t)y)\leq tf(x)+(1-t)f(y).$$

这正是 $f$ 为凸函数的定义。
<!-- endanswer -->

<!-- answer id="review-04" -->
## 例 4　由对偶解求原问题解

把 $y^*=(6/5,1/5)^T$ 代入四条对偶约束，左端依次为 $8/5,13/5,3,4$。前两条严格大于目标系数 $1,2$，由互补松弛得 $x_1^*=x_2^*=0$。

又因 $y_1^*,y_2^*>0$，原问题的两条约束都取等号：

$$2x_3^*+3x_4^*=20,\qquad3x_3^*+2x_4^*=20.$$

解得 $x_3^*=x_4^*=4$。因此

$$\boxed{x^*=(0,0,4,4)^T,\qquad\max Z=28.}$$

对偶目标值为 $20(6/5+1/5)=28$，与原目标一致。
<!-- endanswer -->

<!-- answer id="review-05" -->
## 例 5　由最优解确定参数

对偶为

$$\begin{aligned}
\max\quad&by_1+4y_2,\\
\text{s.t.}\quad&y_1+y_2\leq5,\quad-y_1+y_2\leq0,\\
&3y_1+2y_2\leq14,\qquad y_1,y_2\geq0.
\end{aligned}$$

已知原最优解中 $x_1^*=2>0$、$x_3^*=1>0$，对应两条对偶约束必须取等号：

$$y_1+y_2=5,\qquad3y_1+2y_2=14.$$

解得 $y^*=(4,1)^T$，并且它满足剩余对偶约束。因 $y_1^*=4>0$，原问题第一条约束取等号，故

$$\boxed{b=2-0+3=5,\qquad y^*=(4,1)^T.}$$

两边目标值分别为 $5\cdot2+14=24$、$5\cdot4+4=24$，相等。
<!-- endanswer -->

<!-- answer id="review-06" -->
## 例 6　由对偶证明最优值

**（1）写对偶。** 把第二组约束写成 $-A^Ty\geq-c$。对两组约束分别引入非负乘子 $u,v$，得到

$$\begin{aligned}
(D)\quad\max\quad&b^Tu-c^Tv,\\
\text{s.t.}\quad&A^Tu\leq c,\qquad Av\geq b,\\
&u\geq0,\qquad v\geq0.
\end{aligned}$$

**（2）证明。** 若 $(P)$ 不可行，第一种情况已经成立。若可行，对任意可行 $(x,y)$，

$$c^Tx-b^Ty=x^T(c-A^Ty)+y^T(Ax-b)\geq0.$$

所以 $(P)$ 有下界。线性规划可行且目标有界时能取得最优值，记为 $p^*$。其对偶也有最优解，且 $d^*=p^*$。

另一方面，$(x,y)$ 为 $(P)$ 的可行解，当且仅当 $(u,v)=(y,x)$ 为 $(D)$ 的可行解，两边目标恰好互为相反数。因此 $d^*=-p^*$。

结合两式，$p^*=-p^*$，故 $\boxed{p^*=0}$。
<!-- endanswer -->

<!-- answer id="review-07" -->
## 例 7　两阶段法与对偶问题

**（1）恢复原问题。** 由表 I 读出

$$A=\begin{pmatrix}1&2&1&3\\1&1&2&4\end{pmatrix}.$$

表 II 的基变量依次为 $x_4,x_3$，故

$$B=\begin{pmatrix}3&1\\4&2\end{pmatrix},\qquad
\bar b=\begin{pmatrix}1/2\\3/2\end{pmatrix},\qquad
b=B\bar b=\begin{pmatrix}3\\5\end{pmatrix}.$$

所以原问题为 $\min(2x_1+3x_2+2x_3+2x_4)$，约束为

$$x_1+2x_2+x_3+3x_4=3,\qquad
x_1+x_2+2x_3+4x_4=5,\qquad x\geq0.$$

人工变量之和已为零，可以恢复原目标。取 $c_B=(2,2)^T$，有

$$c_B^TB^{-1}=(-2,2),\qquad q=c_B^TB^{-1}A-c^T=(-2,-5,0,0).$$

所有检验数不大于零，故 $\boxed{x^*=(0,0,3/2,1/2)^T,\ f^*=4}$。

**（2）求对偶解。** 等式约束对应自由变量 $w_1,w_2$，对偶为

$$\begin{aligned}
\max\quad&3w_1+5w_2,\\
\text{s.t.}\quad&w_1+w_2\leq2,\quad2w_1+w_2\leq3,\\
&w_1+2w_2\leq2,\quad3w_1+4w_2\leq2,\\
&w_1,w_2\in\mathbb R.
\end{aligned}$$

由上面的 $c_B^TB^{-1}$ 得 $\boxed{w^*=(-2,2)^T}$。它满足全部对偶约束，目标为 $-6+10=4$，与原问题相等。
<!-- endanswer -->

<!-- answer id="review-08" -->
## 例 8　补全最优单纯形表

三条约束加松弛变量后，按 $x_4,x_1,x_2$ 的顺序取基：

$$B=\begin{pmatrix}1&3&1\\0&1&-1\\0&1&1\end{pmatrix},\qquad
B^{-1}=\begin{pmatrix}1&-1&-2\\0&1/2&1/2\\0&-1/2&1/2\end{pmatrix}.$$

所以 $b=B(10,15,5)^T=(60,10,20)^T$，即

$$\boxed{b_1=60,\quad b_2=10,\quad b_3=20.}$$

将各列左乘 $B^{-1}$，即可补全表格。本题保留原最大化目标，检验数取 $r=c-z$，右上角记原目标值：

$$\begin{array}{c|rrrrrr|r}
&x_1&x_2&x_3&x_4&x_5&x_6&\text{右端}\\\hline
r&0&0&-3/2&0&-3/2&-1/2&25\\\hline
x_4&0&0&1&1&-1&-2&10\\
x_1&1&0&1/2&0&1/2&1/2&15\\
x_2&0&1&-3/2&0&-1/2&1/2&5
\end{array}$$

读出原最优解 $x^*=(15,5,0)^T$，最大值为 $25$。对偶为

$$\begin{aligned}
\min\quad&60y_1+10y_2+20y_3,\\
\text{s.t.}\quad&3y_1+y_2+y_3\geq2,\\
&y_1-y_2+y_3\geq-1,\\
&y_1+2y_2-y_3\geq1,\qquad y\geq0.
\end{aligned}$$

原最大化目标的 $c_B=(0,2,-1)^T$，故 $y^{*T}=c_B^TB^{-1}=(0,3/2,1/2)$。它可行，且目标值为 $15+10=25$，因此 $\boxed{y^*=(0,3/2,1/2)^T}$。
<!-- endanswer -->

<!-- answer id="review-09" -->
## 例 9　Lagrange 对偶与共轭关系

**（1）求对偶。** 对 $x-b\geq0$ 引入 $\lambda\geq0$：

$$L(x,\lambda)=\frac12x^TQx-\lambda^T(x-b).$$

由于 $Q\succ0$，对固定的 $\lambda$，$L$ 关于 $x$ 的唯一最小值点满足 $Qx-\lambda=0$，即 $x=Q^{-1}\lambda$。代入得

$$\theta(\lambda)=b^T\lambda-\frac12\lambda^TQ^{-1}\lambda.$$

所以对偶为 $\boxed{\max_{\lambda\geq0}\{b^T\lambda-\lambda^TQ^{-1}\lambda/2\}}$。

**（2）证明共轭关系。** 原问题为凸规划，并且存在严格可行点 $x=b+\mathbf1$。最优解满足 KKT 条件：

$$Qx^*=\lambda^*,\quad x^*-b\geq0,\quad\lambda^*\geq0,
\quad (\lambda^*)^T(x^*-b)=0.$$

于是

$$\boxed{(x^*)^TQ(x^*-b)=(\lambda^*)^T(x^*-b)=0.}$$

这就是题目要求的 $Q$ 正交关系；若其中某个向量为零，该等式也成立。
<!-- endanswer -->

<!-- answer id="review-10" -->
## 例 10　一阶必要条件与二阶充分条件

沿用正文的减号记法，取

$$L=x_1x_2-\lambda(2x_1-x_2-3).$$

KKT 条件为 $x_2-2\lambda=0$、$x_1+\lambda=0$、$2x_1-x_2-3=0$。在 $\bar x=(3/4,-3/2)^T$ 处取 $\bar\lambda=-3/4$，三式全部成立。约束梯度 $(2,-1)^T\ne0$，线性无关条件满足。

临界方向满足 $2d_1-d_2=0$。Lagrange 函数的海塞矩阵为

$$H=\begin{pmatrix}0&1\\1&0\end{pmatrix}.$$

对任意非零临界方向，$d_2=2d_1$ 且 $d_1\ne0$，所以

$$d^THd=2d_1d_2=4d_1^2>0.$$

二阶充分条件成立，故 $\boxed{\bar x\text{ 是严格局部最优解}}$。

再消去 $x_2=2x_1-3$，原目标为 $2(x_1-3/4)^2-9/8$，可核对它也是唯一全局最优解，最优值为 $-9/8$。
<!-- endanswer -->

<!-- answer id="review-11" -->
## 例 11　无约束极值计算

先考虑 $y=0$。目标为 $-\|x\|_2^p/p\leq0$，在 $x=0$ 处取到最大值 $0$，结论成立。

当 $y\ne0$ 时，记 $s=\|y\|_2>0$、$r=\|x\|_2$。由 Cauchy–Schwarz 不等式，

$$x^Ty-\frac1p\|x\|_2^p\leq rs-\frac1p r^p.$$

对 $r\geq0$ 求最大值。一阶条件 $s-r^{p-1}=0$ 给出

$$r=s^{1/(p-1)}=s^{q-1}.$$

一元函数的导数在此点之前为正、之后为负，所以它是全局最大值点。取 $x=(r/s)y$，上面的不等式取得等号，因此

$$f(y)=s^q-\frac1p s^{p(q-1)}
=\left(1-\frac1p\right)s^q
=\boxed{\frac1q\|y\|_2^q}.$$

这里用到了 $q=p/(p-1)$、$p(q-1)=q$。
<!-- endanswer -->

<!-- answer id="review-12" -->
## 例 12　最速下降法迭代两步

梯度为 $g(x)=(2x_1,4x_2)^T$，海塞矩阵为 $A=\operatorname{diag}(2,4)$。取 $d_k=-g_k$，精确步长为 $\alpha_k=g_k^Tg_k/(g_k^TAg_k)$。

**第一步。** 在 $x^{(0)}=(1,1)^T$，有

$$g_0=(2,4)^T,\qquad\alpha_0=\frac{4+16}{8+64}=\frac5{18}.$$

所以

$$x^{(1)}=x^{(0)}-\alpha_0g_0
=\left(\frac49,-\frac19\right)^T.$$

**第二步。** 此时 $g_1=(8/9,-4/9)^T$，

$$\alpha_1=\frac{(64+16)/81}{(128+64)/81}=\frac5{12}.$$

故

$$\boxed{x^{(2)}=\left(\frac49,-\frac19\right)^T
-\frac5{12}\left(\frac89,-\frac49\right)^T
=\left(\frac2{27},\frac2{27}\right)^T.}$$

两步的步长不同。每一步都要用当前梯度重新计算。
<!-- endanswer -->

<!-- answer id="review-13" -->
## 例 13　FR 共轭梯度法的下一步

精确线搜索后，新梯度与上一步搜索方向正交，因此

$$(-2,-3,\beta)\cdot(2,-1,1)=-4+3+\beta=0.$$

故题目中的未知分量为 $\boxed{\beta=1}$。为避免与 FR 系数混淆，下面把 FR 系数记为 $\eta$。

第一次搜索方向是负梯度，所以 $g_1=-d^{(1)}=(-2,1,-1)^T$。新梯度为 $g_2=(-2,-3,1)^T$，于是

$$\eta=\frac{\|g_2\|^2}{\|g_1\|^2}
=\frac{4+9+1}{4+1+1}=\frac73.$$

新的搜索方向为

$$\boxed{d^{(2)}=-g_2+\eta d^{(1)}
=(2,3,-1)^T+\frac73(2,-1,1)^T
=\left(\frac{20}3,\frac23,\frac43\right)^T.}$$
<!-- endanswer -->

<!-- answer id="review-14" -->
## 例 14　外点罚函数法

构造平方罚函数

$$P_\mu(x)=x_1^2+x_2^2+\mu(x_1+x_2-1)^2,\qquad\mu>0.$$

令梯度为零：

$$2x_1+2\mu(x_1+x_2-1)=0,\qquad
2x_2+2\mu(x_1+x_2-1)=0.$$

两式相减得到 $x_1=x_2$，代回得

$$x_1(\mu)=x_2(\mu)=\frac{\mu}{1+2\mu}.$$

罚函数的海塞矩阵为 $2I+2\mu\mathbf1\mathbf1^T\succ0$，因此该驻点是唯一全局最小值点。令 $\mu\to\infty$，得到

$$\boxed{x^*=(1/2,1/2)^T,\qquad f^*=1/2.}$$

原约束下 $(x_1+x_2)^2\leq2(x_1^2+x_2^2)$，所以目标值至少为 $1/2$；上面的点正好达到这个下界。
<!-- endanswer -->
