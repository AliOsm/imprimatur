# frozen_string_literal: true

require_relative 'test_helper'

class BarcodeTest < Minitest::Test
  # The decompressed content streams of a PDF, one per page.
  def pdf_pages(bytes)
    bytes.scan(/stream\n(.*?)\nendstream/m).map { Zlib::Inflate.inflate(it.first).force_encoding(Encoding::BINARY) }
  end

  # A PNG's width, height, bit depth and color type, and whether it has a tRNS chunk.
  def png_header(bytes)
    width, height, depth, color = bytes.byteslice(16, 10).unpack('NNCC')
    [width, height, depth, color, bytes.include?('tRNS')]
  end

  def test_encodes_every_symbology
    # Linear codes' widths are their modules; matrix codes are square.
    cases = {
      [:qr, 'https://id.gs1.org/01/09506000134352', {}] => [:qr, false, nil],
      [:qr, '(01)09506000134352(10)AB12', { gs1: true, level: :h }] => [:qr, true, nil],
      [:datamatrix, 'CS-2026-0001', {}] => [:datamatrix, false, nil],
      [:datamatrix, '(01)09506000134352(17)251231', { gs1: true }] => [:datamatrix, true, nil],
      [:ean13, '950600013435', {}] => [:ean13, false, 95],
      [:ean8, '9638507', {}] => [:ean8, false, 67],
      [:upca, '03600029145', {}] => [:upca, false, 95],
      [:code128, 'CS-ORDER-000042', {}] => [:code128, false, 178],
      [:code128, '(00)106141412345678908', { gs1: true }] => [:code128, true, 156]
    }
    cases.each do |(symbology, data, options), (kind, gs1, width)|
      code = Imprimatur.barcode(symbology, data, **options)

      assert_equal [kind, gs1, width || code.height], [code.symbology, code.gs1?, code.width], data
      assert_equal 1, code.height if width
      assert_equal code.height, code.modules.size
      assert_equal code.width, code.modules.first.size
    end
    assert_equal '9506000134352', Imprimatur.barcode(:ean13, '950600013435').text
    assert_equal '036000291452', Imprimatur.barcode('upca', '03600029145').text
    assert_equal '#<Imprimatur::Barcode code128 gs1 "(00)106141412345678908">',
                 Imprimatur.barcode(:code128, '(00)106141412345678908', gs1: true).inspect
  end

  def test_draws_svg_and_png
    qr = Imprimatur.barcode(:qr, 'A', level: :l)
    svg = qr.svg

    assert svg.start_with?('<svg xmlns="http://www.w3.org/2000/svg" width="116" height="116" viewBox="0 0 29 29"')
    assert_includes svg, '<rect width="29" height="29" fill="#FFFFFF"/><path fill="#000000" d="M4 4h7v1h-7z'
    bare = qr.svg(module_size: 2.5, color: '#1F3864', background: nil, quiet_zone: false)

    assert_includes bare, 'width="52.5"'
    refute_includes bare, '<rect'

    png = qr.png(module_size: 10)

    assert_equal Encoding::BINARY, png.encoding
    assert_equal [290, 290, 1, 3, false], png_header(png)
    assert_equal [21, 21, 1, 3, true], png_header(qr.png(module_size: 1, background: nil, quiet_zone: false))
    ean = Imprimatur.barcode(:ean13, '9506000134352')

    assert_equal [113, 69, 1, 3, false], png_header(ean.png(module_size: 1))
    assert_equal [95 * 2, 20 * 2, 1, 3, false], png_header(ean.png(module_size: 2, quiet_zone: false, bar_height: 20))
  end

  def test_places_barcodes_in_documents
    qr = Imprimatur.barcode(:qr, 'https://example.com')
    pdf = Imprimatur.pdf(page_size: [200, 200], margin: 10) { it.barcode qr, width: 180, align: :center }
    page = pdf_pages(pdf).first
    module_size = format('%.4f', 180.0 / (qr.width + 8))

    assert page.start_with?("q 0 0 0 rg #{module_size} 0 0 -#{module_size} 10 190 cm\n4 4 7 1 re\n"), page
    assert page.end_with?("f Q\n")

    docx = Imprimatur.docx do |doc|
      doc.barcode Imprimatur.barcode(:ean13, '9506000134352'), width: 113, height: 50, color: '#1F3864'
    end
    parts = ZipReader.entries(docx)

    assert_includes parts.keys, 'word/media/image1.png'
    assert_includes parts['word/document.xml'], '<wp:extent cx="1435100" cy="635000"/>'
  end

  def test_rejects_what_does_not_fit
    errors = {
      -> { Imprimatur.barcode(:aztec, 'x') } => [ArgumentError, 'symbology must be one of :qr'],
      -> { Imprimatur.barcode(:ean13, 'x', gs1: true) } =>
        [ArgumentError, 'gs1: applies to :qr, :datamatrix and :code128'],
      -> { Imprimatur.barcode(:code128, 'x', level: :h) } => [ArgumentError, 'level: applies to :qr only'],
      -> { Imprimatur.barcode(:qr, 'x', level: :z) } => [ArgumentError, 'level must be one of :l, :m, :q, :h, not :z'],
      -> { Imprimatur.barcode(:ean13, '9506000134353') } =>
        [Imprimatur::Error, 'invalid EAN-13 barcode: 9506000134353 has check digit 3, not 2'],
      -> { Imprimatur.barcode(:code128, '(01)0950600013435', gs1: true) } =>
        [Imprimatur::Error, 'invalid GS1 element string: (01) takes 14 digits, not "0950600013435"'],
      -> { Imprimatur.barcode(:code128, 'x').png(module_size: 0) } => [Imprimatur::Error, 'a scale of 1 or more'],
      -> { Imprimatur.barcode(:code128, 'x').svg(color: 'red') } => [Imprimatur::Error, 'invalid color "red"'],
      -> { Imprimatur.pdf { it.barcode 'not a barcode' } } => [TypeError, 'Imprimatur::Barcode']
    }
    errors.each do |call, (error, message)|
      raised = assert_raises(error) { call.call }

      assert_includes raised.message, message
    end
  end
end
