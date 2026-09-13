/* Replaces only minicheck's runner. Each test's setup/body/teardown execute in
 * one child; assertion failures terminate that child without a longjmp over
 * live Rust frames. The parent records failures, signals and timeouts. */
void
srunner_run_all(SRunner *runner, const char *context, int verbosity) {
  (void)verbosity;
  for (TCase *tc = runner->suite->tests; tc; tc = tc->next_tcase) {
    for (int i = 0; i < tc->ntests; ++i) {
      const char *name = oriole_test_name(tc->tests[i]);
      if (!oriole_test_enabled(name))
        continue;
      ++runner->nchecks;
      printf("ORIOLE_BEGIN\t%s\t%s\n", context, name);
      fflush(NULL);
      pid_t child = fork();
      if (child < 0) {
        perror("fork");
        exit(2);
      }
      if (child == 0) {
        const char *memory_text = getenv("ORIOLE_MEMORY_MIB");
        rlim_t bytes = (memory_text ? strtoull(memory_text, NULL, 10) : 1024)
                       * 1024ULL * 1024;
        struct rlimit memory = {bytes, bytes};
        struct rlimit core = {0, 0};
        if (setrlimit(RLIMIT_AS, &memory) || setrlimit(RLIMIT_CORE, &core))
          _Exit(101);
        const char *timeout = getenv("ORIOLE_TEST_TIMEOUT");
        alarm(timeout ? (unsigned)atoi(timeout) : 3);
        set_subtest("%s", "");
        if (tc->setup)
          tc->setup();
        tc->tests[i]();
        if (tc->teardown)
          tc->teardown();
        fflush(NULL);
        _Exit(0);
      }
      int status, rss_limited = 0;
      unsigned polls = 0;
      const char *rss_text = getenv("ORIOLE_RSS_MIB");
      unsigned long long rss_limit = (rss_text ? strtoull(rss_text, NULL, 10) : 768) * 1024;
      for (;;) {
        pid_t waited = waitpid(child, &status, WNOHANG);
        if (waited == child)
          break;
        if (waited < 0 && errno != EINTR) {
          perror("waitpid");
          exit(2);
        }
        if (polls++ % 10 == 0) {
          char path[80], line[256];
          snprintf(path, sizeof(path), "/proc/%ld/status", (long)child);
          FILE *usage = fopen(path, "r");
          if (usage) {
            while (fgets(line, sizeof(line), usage)) {
              unsigned long long rss;
              if (sscanf(line, "VmRSS: %llu kB", &rss) == 1 && rss > rss_limit) {
                rss_limited = 1;
                kill(child, SIGKILL);
                break;
              }
            }
            fclose(usage);
          }
        }
        usleep(1000);
      }
      int passed = WIFEXITED(status) && WEXITSTATUS(status) == 0;
      if (!passed)
        ++runner->nfailures;
      printf("ORIOLE_RESULT\t%s\t%s\t%s\t%d\n", context, name,
             passed ? "pass" : rss_limited ? "rss-limit"
             : WIFSIGNALED(status) && WTERMSIG(status) == SIGALRM ? "timeout"
             : WIFSIGNALED(status) ? "signal" : "fail",
             WIFSIGNALED(status) ? WTERMSIG(status) : WEXITSTATUS(status));
      fflush(NULL);
    }
  }
}
