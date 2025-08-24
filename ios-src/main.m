// Bridge Rust stdout/stderr to iOS unified logging so Bevy/tracing output
// appears in Xcode
#import <Foundation/Foundation.h>
#include <dispatch/dispatch.h>
#include <stdio.h>
#include <unistd.h>

static void redirect_stdout_stderr_to_nslog(void) {
  int fds[2];
  if (pipe(fds) != 0) {
    return;
  }

  // Make stdout/stderr unbuffered and duplicate to pipe's write end
  setvbuf(stdout, NULL, _IONBF, 0);
  setvbuf(stderr, NULL, _IONBF, 0);
  dup2(fds[1], STDOUT_FILENO);
  dup2(fds[1], STDERR_FILENO);
  close(fds[1]);

  int read_fd = fds[0];
  dispatch_queue_t queue = dispatch_get_global_queue(QOS_CLASS_UTILITY, 0);
  dispatch_source_t source = dispatch_source_create(
      DISPATCH_SOURCE_TYPE_READ, (uintptr_t)read_fd, 0, queue);
  if (!source) {
    close(read_fd);
    return;
  }

  dispatch_source_set_event_handler(source, ^{
    @autoreleasepool {
      char buffer[1024];
      ssize_t n = read(read_fd, buffer, sizeof(buffer) - 1);
      if (n > 0) {
        buffer[n] = '\0';
        NSString *str = [[NSString alloc] initWithBytes:buffer
                                                 length:(NSUInteger)n
                                               encoding:NSUTF8StringEncoding];
        if (!str) {
          str = [NSString stringWithCString:buffer
                                   encoding:NSASCIIStringEncoding];
        }
        if (str) {
          NSCharacterSet *nl = [NSCharacterSet newlineCharacterSet];
          NSArray<NSString *> *lines =
              [str componentsSeparatedByCharactersInSet:nl];
          for (NSString *line in lines) {
            if (line.length > 0) {
              NSLog(@"%@", line);
            }
          }
        }
      }
    }
  });

  dispatch_source_set_cancel_handler(source, ^{
    close(read_fd);
  });
  dispatch_resume(source);
}

extern void rust_main(void);
int main(int argc, char *argv[]) {
  redirect_stdout_stderr_to_nslog();
  rust_main();
  return 0;
}
