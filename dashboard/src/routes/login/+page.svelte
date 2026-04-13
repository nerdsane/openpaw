<script lang="ts">
  import { goto } from '$app/navigation';
  import { base } from '$app/paths';
  import { onMount } from 'svelte';
  import { getAuthProviders, login, register } from '$lib/auth';

  let mode = $state<'login' | 'register'>('login');
  let email = $state('');
  let password = $state('');
  let confirmPassword = $state('');
  let loading = $state(true);
  let submitting = $state(false);
  let error = $state('');

  onMount(async () => {
    try {
      const providers = await getAuthProviders();
      mode = providers.registration_open ? 'register' : 'login';
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not load authentication options';
    } finally {
      loading = false;
    }
  });

  async function submit() {
    if (!email.trim()) {
      error = 'Email is required';
      return;
    }
    if (!password.trim()) {
      error = 'Password is required';
      return;
    }
    if (mode === 'register' && password !== confirmPassword) {
      error = 'Passwords do not match';
      return;
    }

    submitting = true;
    error = '';

    try {
      if (mode === 'register') {
        await register(email, password);
      } else {
        await login(email, password);
      }
      await goto(`${base}/`);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Authentication failed';
    } finally {
      submitting = false;
    }
  }

  function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    void submit();
  }
</script>

<svelte:head>
  <title>Open Paw Login</title>
</svelte:head>

<section class="login-shell">
  <div class="login-card">
    <p class="eyebrow">OPEN PAW</p>
    <h1>{mode === 'register' ? 'Create your account' : 'Welcome back'}</h1>
    <p class="lede">
      {mode === 'register'
        ? 'This deployment is still unclaimed. Create the first admin account to unlock the dashboard.'
        : 'Sign in with the admin account for this deployment.'}
    </p>

    {#if loading}
      <p class="status">Loading authentication options…</p>
    {:else}
      <form class="form" onsubmit={handleSubmit}>
        <label>
          <span>Email</span>
          <input bind:value={email} autocomplete="email" placeholder="you@example.com" type="email" />
        </label>

        <label>
          <span>Password</span>
          <input bind:value={password} autocomplete={mode === 'register' ? 'new-password' : 'current-password'} placeholder="••••••••" type="password" />
        </label>

        {#if mode === 'register'}
          <label>
            <span>Confirm password</span>
            <input bind:value={confirmPassword} autocomplete="new-password" placeholder="••••••••" type="password" />
          </label>
        {/if}

        {#if error}
          <p class="error">{error}</p>
        {/if}

        <button class="submit" disabled={submitting} type="submit">
          {submitting ? 'Working…' : mode === 'register' ? 'Create account' : 'Sign in'}
        </button>
      </form>
    {/if}
  </div>
</section>

<style>
  .login-shell {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 2rem;
    background:
      radial-gradient(circle at top left, rgba(83, 160, 255, 0.18), transparent 35%),
      radial-gradient(circle at bottom right, rgba(255, 196, 72, 0.18), transparent 30%),
      var(--bg);
  }

  .login-card {
    width: min(440px, 100%);
    padding: 2rem;
    border: 1px solid var(--border);
    border-radius: 24px;
    background: color-mix(in srgb, var(--bg) 82%, white 4%);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.18);
  }

  .eyebrow {
    margin: 0 0 0.75rem;
    font-size: 0.75rem;
    letter-spacing: 0.12em;
    color: var(--text-3);
  }

  h1 {
    margin: 0;
    font-size: clamp(2rem, 4vw, 2.6rem);
  }

  .lede {
    margin: 0.75rem 0 1.5rem;
    color: var(--text-2);
    line-height: 1.5;
  }

  .form {
    display: grid;
    gap: 1rem;
  }

  label {
    display: grid;
    gap: 0.35rem;
  }

  label span {
    font-size: 0.85rem;
    color: var(--text-2);
  }

  input {
    width: 100%;
    padding: 0.85rem 1rem;
    border-radius: 14px;
    border: 1px solid var(--border);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-1);
  }

  .submit {
    margin-top: 0.5rem;
    padding: 0.95rem 1rem;
    border-radius: 14px;
    border: none;
    background: linear-gradient(135deg, #53a0ff, #7a6cff);
    color: white;
    font-weight: 600;
  }

  .status,
  .error {
    margin: 0;
    font-size: 0.9rem;
  }

  .error {
    color: #ff8b8b;
  }
</style>
