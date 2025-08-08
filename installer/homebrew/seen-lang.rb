class SeenLang < Formula
  desc "High-performance systems programming language"
  homepage "https://seen-lang.org"
  url "https://github.com/seen-lang/seen/releases/download/v1.0.0/seen-1.0.0-macos-x64.tar.gz"
  sha256 "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
  license "MIT"
  version "1.0.0"

  # Dependencies
  depends_on "llvm" => :build
  depends_on "cmake" => :build

  # Runtime dependencies
  depends_on "libffi"

  # Supported platforms
  on_macos do
    if Hardware::CPU.intel?
      url "https://github.com/seen-lang/seen/releases/download/v1.0.0/seen-1.0.0-macos-x64.tar.gz"
      sha256 "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    elsif Hardware::CPU.arm?
      url "https://github.com/seen-lang/seen/releases/download/v1.0.0/seen-1.0.0-macos-arm64.tar.gz" 
      sha256 "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/seen-lang/seen/releases/download/v1.0.0/seen-1.0.0-linux-x64.tar.gz"
      sha256 "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
    elsif Hardware::CPU.arm? && Hardware::CPU.is_64_bit?
      url "https://github.com/seen-lang/seen/releases/download/v1.0.0/seen-1.0.0-linux-arm64.tar.gz"
      sha256 "9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba"
    end
  end

  def install
    # Install binaries
    bin.install "seen"
    bin.install "seen-lsp" if File.exist?("seen-lsp")
    bin.install "seen-riscv" if File.exist?("seen-riscv")

    # Install standard library
    if Dir.exist?("stdlib")
      lib.mkpath
      (lib/"seen").mkpath
      cp_r "stdlib/.", lib/"seen"
    end

    # Install language configurations
    if Dir.exist?("languages")
      share.mkpath
      (share/"seen").mkpath
      cp_r "languages", share/"seen"
    end

    # Install documentation
    if Dir.exist?("docs")
      doc.mkpath
      cp_r "docs/.", doc
    end

    # Install man pages
    if File.exist?("seen.1")
      man1.install "seen.1"
    end

    # Create shell completions
    generate_completions_from_executable(bin/"seen", "completion", shells: [:bash, :zsh, :fish])
  end

  def post_install
    # Set up environment
    ENV["SEEN_LIB_PATH"] = lib/"seen"
    ENV["SEEN_DATA_PATH"] = share/"seen"
    
    # Print success message
    ohai "Seen Language installed successfully!"
    ohai "Run 'seen --version' to verify installation."
    ohai "Documentation: https://docs.seen-lang.org"
  end

  test do
    # Test basic functionality
    system bin/"seen", "--version"
    
    # Test project creation and building
    mkdir "test-project" do
      system bin/"seen", "init", "hello"
      cd "hello" do
        # Check if Seen.toml was created
        assert_predicate Pathname.pwd/"Seen.toml", :exist?
        
        # Try to build the project
        system bin/"seen", "build"
      end
    end

    # Test LSP server if available
    if (bin/"seen-lsp").exist?
      # Basic LSP test - just check it starts
      pid = spawn bin/"seen-lsp", "--stdio"
      sleep 2
      Process.kill("TERM", pid)
    end
  end
end

# Additional formula configurations
class SeenLang
  # Caveats for users
  def caveats
    <<~EOS
      Seen Language has been installed successfully!

      To get started:
        seen init my-project
        cd my-project  
        seen build
        seen run

      VS Code Extension:
        Install the "Seen Language" extension from the VS Code marketplace
        for syntax highlighting, IntelliSense, and debugging support.

      Language Server:
        The language server (seen-lsp) is included for IDE integration.
        Most editors will auto-discover it when the extension is installed.

      Documentation:
        - Language reference: https://docs.seen-lang.org
        - API documentation: https://api.seen-lang.org
        - Community: https://discord.gg/seen-lang

      Environment Variables:
        SEEN_LIB_PATH: #{lib}/seen (standard library location)
        SEEN_DATA_PATH: #{share}/seen (language configurations)

      If you encounter any issues:
        - Check: brew doctor
        - Report bugs: https://github.com/seen-lang/seen/issues
    EOS
  end

  # Bottle configuration for binary distribution
  bottle do
    root_url "https://github.com/seen-lang/seen/releases/download/v1.0.0/"
    
    sha256 cellar: :any_skip_relocation, arm64_ventura:  "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
    sha256 cellar: :any_skip_relocation, arm64_monterey: "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
    sha256 cellar: :any_skip_relocation, arm64_big_sur:  "fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321"
    sha256 cellar: :any_skip_relocation, ventura:        "9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba"
    sha256 cellar: :any_skip_relocation, monterey:       "0fedcba9876543210fedcba9876543210fedcba9876543210fedcba987654321"
    sha256 cellar: :any_skip_relocation, big_sur:        "ba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedc"
    sha256 cellar: :any_skip_relocation, x86_64_linux:   "4321fedcba9876543210fedcba9876543210fedcba9876543210fedcba987654"
  end

  # Resource definitions for additional files
  resource "language-configs" do
    url "https://github.com/seen-lang/seen/releases/download/v1.0.0/language-configs.tar.gz"
    sha256 "567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234"
  end

  resource "stdlib-source" do
    url "https://github.com/seen-lang/seen/releases/download/v1.0.0/stdlib-source.tar.gz"
    sha256 "cdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890ab"
  end
end