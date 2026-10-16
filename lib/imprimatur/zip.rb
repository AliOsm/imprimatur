# frozen_string_literal: true

require 'fileutils'

module Imprimatur
  # Zip archives of any files: entries written with their modification times
  # and comments (ZIP64 when large), read one at a time within a size limit.
  # Compressing and reading run without the GVL.
  #
  #   Imprimatur::Zip.write('export.zip') do |zip|
  #     zip.add_file('contracts/0contract.pdf', '/tmp/contract.pdf', comment: 'Contract')
  #     zip.add('manifest.json', JSON.generate(manifest))
  #   end
  #
  #   Imprimatur::Zip.open('geolite.zip') do |zip|
  #     zip.read(zip.glob('**/*-Locations-en.csv').first)
  #   end
  module Zip
    # Bytes an entry may hold once decompressed by default: 1 GiB.
    MAX_ENTRY_BYTES = 1024**3

    # Writes an archive at +path+, the block adding its entries. On an
    # error, the partial file is removed.
    #
    # @yieldparam zip [Writer]
    # @return the path
    def self.write(path)
      writer = Writer.new(path)
      yield writer
      writer.finish
      path
    rescue Exception # any failure, an interrupt included: the partial file goes, then the error
      FileUtils.rm_f(path.to_s) if writer
      raise
    end

    # The archive at +path+.
    #
    # @yieldparam zip [Reader] when a block is given
    # @return [Reader, Object] the reader, or what the block returns
    # @raise [Errno::ENOENT] when there is no such file
    # @raise [Error] when it is no zip
    def self.open(path)
      reader = Reader.new(path)
      block_given? ? yield(reader) : reader
    end

    # Adds entries to an archive being written; see {Zip.write}.
    class Writer
      def initialize(path)
        @native = Native::ZipWriter.new(path.to_s)
      end

      # Adds the file at +source+ as +name+, streamed from disk, with its
      # modification time.
      def add_file(name, source, comment: nil)
        @native.add_file(name.to_s, source.to_s, comment&.to_s)
        self
      end

      # Adds +data+ as +name+, dated now.
      def add(name, data, comment: nil)
        @native.add(name.to_s, data.to_s.b, comment&.to_s)
        self
      end

      # @api private
      def finish = @native.finish
    end

    # An archive's entries, by name; see {Zip.open}.
    class Reader
      # @return [Array<String>] the entries' names, in the archive's order
      attr_reader :names

      def initialize(path)
        raise Errno::ENOENT, path.to_s unless File.file?(path.to_s)

        @native = Native::ZipReader.new(path.to_s)
        @names = @native.names.freeze
      end

      # The names matching a shell pattern, +**/+ spanning folders.
      def glob(pattern) = names.select { File.fnmatch?(pattern, it, File::FNM_PATHNAME | File::FNM_EXTGLOB) }

      # The entry's bytes, binary.
      #
      # @raise [KeyError] for an entry the archive does not have
      # @raise [LimitExceeded] when it holds more than +max_bytes+
      def read(name, max_bytes: MAX_ENTRY_BYTES)
        @native.read(name.to_s, Integer(max_bytes)) or raise KeyError, "no entry #{name} in the archive"
      end

      # Writes the entry to +path+, streamed; returns +path+.
      #
      # @raise (see #read)
      def extract(name, path, max_bytes: MAX_ENTRY_BYTES)
        @native.extract(name.to_s, path.to_s, Integer(max_bytes)) or raise KeyError, "no entry #{name} in the archive"
        path
      end
    end
  end
end
