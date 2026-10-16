# frozen_string_literal: true

require_relative 'test_helper'

class ZipTest < Minitest::Test
  def test_writes_and_reads_archives
    Dir.mktmpdir do |dir|
      source = File.join(dir, 'contract.pdf')
      File.binwrite(source, "%PDF-1.7 #{'x' * 50_000}")
      File.utime(Time.utc(2023, 3, 15, 9, 30), Time.utc(2023, 3, 15, 9, 30), source)
      path = File.join(dir, 'export.zip')

      added = nil
      written = Imprimatur::Zip.write(path) do |zip|
        zip.add_file('contracts/0contract.pdf', source, comment: 'Contract - signed')
        zip.add('manifest.json', '{"items":1}')
        zip.comment = 'An export'
        added = zip.entries
      end

      assert_equal [['contracts/0contract.pdf', 'Contract - signed'], ['manifest.json', nil]],
                   added.map { [it.name, it.comment] }

      assert_equal path, written
      if system('which unzip', out: File::NULL)
        assert system('unzip', '-tq', path, out: File::NULL), 'unzip reads it'
        assert_match(/03-15-2023 09:30|2023-03-15 09:30/, `unzip -l #{path}`, 'the file keeps its time')
      end

      Imprimatur::Zip.open(path) do |zip|
        assert_equal %w[contracts/0contract.pdf manifest.json], zip.names
        manifest = zip.entries.last

        assert_equal ['manifest.json', 11, Zlib.crc32('{"items":1}'), ''], manifest.to_h.values_at(:name, :size, :crc32, :comment)
        assert_equal ['contracts/0contract.pdf'], zip.glob('**/*.pdf')
        assert_equal '{"items":1}', zip.read('manifest.json')
        assert_equal Encoding::BINARY, zip.read('manifest.json').encoding

        target = File.join(dir, 'out.pdf')

        assert_equal target, zip.extract('contracts/0contract.pdf', target)
        assert_equal File.binread(source), File.binread(target)
        assert_raises(KeyError) { zip.read('missing') }
        assert_raises(Imprimatur::LimitExceeded) { zip.read('contracts/0contract.pdf', max_bytes: 100) }
      end
    end
  end

  def test_refuses_what_is_no_archive
    Dir.mktmpdir do |dir|
      junk = File.join(dir, 'junk.zip')
      File.write(junk, 'not a zip')

      assert_raises(Imprimatur::Error) { Imprimatur::Zip.open(junk) }
      assert_raises(Errno::ENOENT) { Imprimatur::Zip.open(File.join(dir, 'missing.zip')) }
    end
  end

  def test_a_failed_write_leaves_no_file
    Dir.mktmpdir do |dir|
      path = File.join(dir, 'partial.zip')

      assert_raises(RuntimeError) { Imprimatur::Zip.write(path) { |zip| zip.add('a.txt', 'a') && raise('boom') } }
      refute_path_exists path
    end
  end
end
